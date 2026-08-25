use fasti_application::{
    AcceptObservationCommand, AccessAdministrationPort, AppendCorrectionCommand,
    AttachIdentifierCommand, CorrectionPort, CorrectionTarget, CreateRecordCommand,
    EnrollFirstClientCommand, EvidenceUploadPort, EvidenceUploadRequest, IdentityPort,
    InitializeNodeCommand, InspectCorrectionChainQuery, ObservationAcceptancePort, ProblemCode,
    RegisterNamespaceDefinitionCommand, RequestAccessContext, ScopeKey, SecretMaterial,
};
use fasti_domain::{
    ClaimedTrust, ExternalIdentifierClaim, Grain, NamespaceDefinition, NamespaceLicencePosture,
    ObservedAt, OperationId, ProfileGrantId, ProfileId, RequestCorrelationId,
};
use fasti_store::SqliteKernel;
use rusqlite::{params, Connection};
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::TempDir;

struct TestNode {
    _root: TempDir,
    kernel: SqliteKernel,
    access: RequestAccessContext,
}

impl TestNode {
    fn new() -> Self {
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
        let access = *enrolled.access();
        add_correction_scopes(&kernel, access);

        Self {
            _root: root,
            kernel,
            access,
        }
    }

    fn upload(&self, bytes: &[u8]) -> fasti_domain::EvidenceReference {
        let mut upload = self
            .kernel
            .begin_evidence_upload(EvidenceUploadRequest::new(
                RequestCorrelationId::new_v7(),
                self.access,
                Some(bytes.len() as u64),
            ))
            .expect("begin evidence upload");
        upload.write_chunk(bytes).expect("write evidence");
        upload.finish().expect("finish evidence")
    }

    fn create_resolved_observation(&self) -> fasti_domain::ObservationId {
        self.kernel
            .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                RequestCorrelationId::new_v7(),
                self.access,
                NamespaceDefinition::try_new(
                    "imdb",
                    "IMDb title",
                    [Grain::Release],
                    "^tt[0-9]+$",
                    "trim",
                    NamespaceLicencePosture::IdentifiersOnly,
                )
                .expect("valid IMDb test namespace"),
            ))
            .expect("register IMDb namespace");
        let record = self
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                self.access,
                Grain::Release,
            ))
            .expect("create record")
            .record_id();
        let claim = ExternalIdentifierClaim::try_new("imdb", Grain::Release, "tt0903747")
            .expect("valid identifier");
        self.kernel
            .attach_identifier(AttachIdentifierCommand::new(
                RequestCorrelationId::new_v7(),
                self.access,
                record,
                claim.clone(),
            ))
            .expect("attach identifier");
        let evidence = self.upload(b"immutable correction security evidence");
        self.kernel
            .authorize_and_accept(
                AcceptObservationCommand::new(
                    RequestCorrelationId::new_v7(),
                    self.access,
                    OperationId::new_v7(),
                    None,
                    observed_at(),
                    evidence,
                )
                .with_identity_clues(vec![claim], Some(Grain::Release)),
            )
            .expect("accept resolved observation")
            .receipt()
            .observation_id()
    }

    fn create_release_record(&self) -> fasti_domain::RecordId {
        self.kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                self.access,
                Grain::Release,
            ))
            .expect("create replacement record")
            .record_id()
    }

    fn add_profile_with_correction_scopes(&self) -> RequestAccessContext {
        let profile_id = ProfileId::new_v7();
        let grant_id = ProfileGrantId::new_v7();
        let connection = Connection::open(self.kernel.database_path()).expect("open SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        connection
            .execute(
                "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)",
                params![
                    profile_id.to_string(),
                    self.access.workspace_id().to_string(),
                    "2026-08-24T10:00:00Z"
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
                    "2026-08-24T10:00:00Z"
                ],
            )
            .expect("insert profile grant");
        for scope in [ScopeKey::CorrectionRead, ScopeKey::CorrectionWrite] {
            connection
                .execute(
                    "INSERT INTO grant_scopes(grant_id, scope_key) VALUES (?1, ?2)",
                    params![grant_id.to_string(), scope.as_str()],
                )
                .expect("insert correction scope");
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

fn add_correction_scopes(kernel: &SqliteKernel, access: RequestAccessContext) {
    let connection = Connection::open(kernel.database_path()).expect("open SQLite");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enable foreign keys");
    for scope in [ScopeKey::CorrectionRead, ScopeKey::CorrectionWrite] {
        connection
            .execute(
                "INSERT OR IGNORE INTO grant_scopes(grant_id, scope_key) VALUES (?1, ?2)",
                params![access.grant_id().to_string(), scope.as_str()],
            )
            .expect("add staged correction scope");
    }
}

fn observed_at() -> ObservedAt {
    ObservedAt::parse("2026-08-24T10:00:00Z", ClaimedTrust::DeviceObserved).expect("observed time")
}

#[test]
fn correction_rejects_cross_workspace_access() {
    let owner = TestNode::new();
    let foreign = TestNode::new();
    let observation_id = owner.create_resolved_observation();

    let error = owner
        .kernel
        .append_correction(AppendCorrectionCommand::new(
            RequestCorrelationId::new_v7(),
            foreign.access,
            observation_id,
            CorrectionTarget::Unresolved,
            "A foreign workspace must not change this interpretation.",
        ))
        .expect_err("cross-workspace correction must fail");

    assert_eq!(error.code(), ProblemCode::Forbidden);
}

#[test]
fn correction_rejects_cross_profile_observation() {
    let node = TestNode::new();
    let observation_id = node.create_resolved_observation();
    let other_profile = node.add_profile_with_correction_scopes();

    let error = node
        .kernel
        .append_correction(AppendCorrectionCommand::new(
            RequestCorrelationId::new_v7(),
            other_profile,
            observation_id,
            CorrectionTarget::Unresolved,
            "A different profile must not change this interpretation.",
        ))
        .expect_err("cross-profile correction must fail");

    assert_eq!(error.code(), ProblemCode::ValidationFailed);
}

#[test]
fn concurrent_corrections_preserve_a_single_chain_leaf() {
    let node = TestNode::new();
    let observation_id = node.create_resolved_observation();
    let first_record = node.create_release_record();
    let second_record = node.create_release_record();
    let barrier = Arc::new(Barrier::new(3));
    let mut joins = Vec::new();

    for (record_id, reason) in [
        (first_record, "Concurrent correction A"),
        (second_record, "Concurrent correction B"),
    ] {
        let kernel = node.kernel.clone();
        let barrier = Arc::clone(&barrier);
        let access = node.access;
        joins.push(thread::spawn(move || {
            barrier.wait();
            kernel.append_correction(AppendCorrectionCommand::new(
                RequestCorrelationId::new_v7(),
                access,
                observation_id,
                CorrectionTarget::Record(record_id),
                reason,
            ))
        }));
    }

    barrier.wait();
    for join in joins {
        join.join()
            .expect("correction thread")
            .expect("serialized correction");
    }

    let chain = node
        .kernel
        .inspect_correction_chain(InspectCorrectionChainQuery::new(
            RequestCorrelationId::new_v7(),
            node.access,
            observation_id,
        ))
        .expect("inspect correction chain");
    assert_eq!(chain.corrections().len(), 2);

    let connection = Connection::open(node.kernel.database_path()).expect("open SQLite");
    let leaf_count = connection
        .query_row(
            r#"
            SELECT COUNT(*) FROM interpretations i
            WHERE i.observation_id = ?1
              AND NOT EXISTS(
                  SELECT 1 FROM interpretations child
                  WHERE child.prior_interpretation_id = i.interpretation_id
              )
            "#,
            [observation_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .expect("count chain leaves");
    assert_eq!(leaf_count, 1);
}
