//! B2 local-kernel commands, queries, outcomes, and adapter ports.

use crate::{ApplicationResult, RequestAccessContext};
use fasti_domain::{
    EvidenceReference, ExternalIdentifierClaim, ExternalIdentifierId, Grain, InterpretationId,
    ProfileId, RecordId, ReviewItemId, ReviewStatus, WorkspaceId,
};
use std::fmt;

const SECRET_BYTES: usize = 32;
const SECRET_HEX_BYTES: usize = SECRET_BYTES * 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretParseError;

impl fmt::Display for SecretParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("secret must contain exactly 64 lowercase hexadecimal characters")
    }
}

impl std::error::Error for SecretParseError {}

/// One-time secret material.
///
/// This type deliberately implements neither `Debug`, `Clone`, nor
/// serialization. Delivery adapters may expose it only in the one successful
/// response that creates or rotates the credential.
pub struct SecretMaterial {
    bytes: [u8; SECRET_BYTES],
}

impl SecretMaterial {
    pub fn from_bytes(bytes: [u8; SECRET_BYTES]) -> Self {
        Self { bytes }
    }

    pub fn try_from_hex(value: &str) -> Result<Self, SecretParseError> {
        if value.len() != SECRET_HEX_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(SecretParseError);
        }
        let mut bytes = [0_u8; SECRET_BYTES];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (decode_hex(pair[0])? << 4) | decode_hex(pair[1])?;
        }
        Ok(Self { bytes })
    }

    pub fn expose_hex(&self) -> String {
        let mut value = String::with_capacity(SECRET_HEX_BYTES);
        for byte in self.bytes {
            use std::fmt::Write as _;
            write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
        }
        value
    }

    pub fn expose_bytes(&self) -> &[u8; SECRET_BYTES] {
        &self.bytes
    }
}

impl Drop for SecretMaterial {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}

fn decode_hex(value: u8) -> Result<u8, SecretParseError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(SecretParseError),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitializeNodeCommand {
    correlation_id: fasti_domain::RequestCorrelationId,
}

impl InitializeNodeCommand {
    pub const fn new(correlation_id: fasti_domain::RequestCorrelationId) -> Self {
        Self { correlation_id }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }
}

pub struct InitializeNodeOutcome {
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    client_id: fasti_domain::ClientId,
    initialization_proof: SecretMaterial,
}

impl InitializeNodeOutcome {
    pub fn new(
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
        client_id: fasti_domain::ClientId,
        initialization_proof: SecretMaterial,
    ) -> Self {
        Self {
            workspace_id,
            profile_id,
            client_id,
            initialization_proof,
        }
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub const fn client_id(&self) -> fasti_domain::ClientId {
        self.client_id
    }

    pub const fn initialization_proof(&self) -> &SecretMaterial {
        &self.initialization_proof
    }
}

pub struct EnrollFirstClientCommand {
    correlation_id: fasti_domain::RequestCorrelationId,
    initialization_proof: SecretMaterial,
}

impl EnrollFirstClientCommand {
    pub const fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        initialization_proof: SecretMaterial,
    ) -> Self {
        Self {
            correlation_id,
            initialization_proof,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn initialization_proof(&self) -> &SecretMaterial {
        &self.initialization_proof
    }
}

pub struct EnrollFirstClientOutcome {
    access: RequestAccessContext,
    credential: SecretMaterial,
}

impl EnrollFirstClientOutcome {
    pub const fn new(access: RequestAccessContext, credential: SecretMaterial) -> Self {
        Self { access, credential }
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn credential(&self) -> &SecretMaterial {
        &self.credential
    }
}

pub struct AuthenticateCredentialQuery {
    correlation_id: fasti_domain::RequestCorrelationId,
    credential: SecretMaterial,
}

impl AuthenticateCredentialQuery {
    pub const fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        credential: SecretMaterial,
    ) -> Self {
        Self {
            correlation_id,
            credential,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn credential(&self) -> &SecretMaterial {
        &self.credential
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotateCredentialCommand {
    correlation_id: fasti_domain::RequestCorrelationId,
    access: RequestAccessContext,
}

impl RotateCredentialCommand {
    pub const fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        access: RequestAccessContext,
    ) -> Self {
        Self {
            correlation_id,
            access,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
}

pub struct RotateCredentialOutcome {
    access: RequestAccessContext,
    credential: SecretMaterial,
}

impl RotateCredentialOutcome {
    pub const fn new(access: RequestAccessContext, credential: SecretMaterial) -> Self {
        Self { access, credential }
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn credential(&self) -> &SecretMaterial {
        &self.credential
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevokeCredentialCommand {
    correlation_id: fasti_domain::RequestCorrelationId,
    access: RequestAccessContext,
    target_credential_id: fasti_domain::CredentialId,
}

impl RevokeCredentialCommand {
    pub const fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        access: RequestAccessContext,
        target_credential_id: fasti_domain::CredentialId,
    ) -> Self {
        Self {
            correlation_id,
            access,
            target_credential_id,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn target_credential_id(&self) -> fasti_domain::CredentialId {
        self.target_credential_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileSelectionOutcome {
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
}

impl ProfileSelectionOutcome {
    pub const fn new(workspace_id: WorkspaceId, profile_id: ProfileId) -> Self {
        Self {
            workspace_id,
            profile_id,
        }
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListenerConfiguration {
    listen: String,
    remote_enabled: bool,
}

impl ListenerConfiguration {
    pub fn new(listen: impl Into<String>, remote_enabled: bool) -> Self {
        Self {
            listen: listen.into(),
            remote_enabled,
        }
    }

    pub fn listen(&self) -> &str {
        &self.listen
    }

    pub const fn remote_enabled(&self) -> bool {
        self.remote_enabled
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigureListenerCommand {
    correlation_id: fasti_domain::RequestCorrelationId,
    access: RequestAccessContext,
    loopback_port: u16,
}

impl ConfigureListenerCommand {
    pub const fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        access: RequestAccessContext,
        loopback_port: u16,
    ) -> Self {
        Self {
            correlation_id,
            access,
            loopback_port,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn loopback_port(&self) -> u16 {
        self.loopback_port
    }
}

pub trait AccessAdministrationPort: Send + Sync {
    fn initialize_node(
        &self,
        command: InitializeNodeCommand,
    ) -> ApplicationResult<InitializeNodeOutcome>;

    fn enroll_first_client(
        &self,
        command: EnrollFirstClientCommand,
    ) -> ApplicationResult<EnrollFirstClientOutcome>;

    fn authenticate_credential(
        &self,
        query: AuthenticateCredentialQuery,
    ) -> ApplicationResult<RequestAccessContext>;

    fn rotate_credential(
        &self,
        command: RotateCredentialCommand,
    ) -> ApplicationResult<RotateCredentialOutcome>;

    fn revoke_credential(&self, command: RevokeCredentialCommand) -> ApplicationResult<()>;

    fn select_profile(
        &self,
        correlation_id: fasti_domain::RequestCorrelationId,
        access: RequestAccessContext,
    ) -> ApplicationResult<ProfileSelectionOutcome>;

    fn configure_listener(
        &self,
        command: ConfigureListenerCommand,
    ) -> ApplicationResult<ListenerConfiguration>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceUploadRequest {
    correlation_id: fasti_domain::RequestCorrelationId,
    access: RequestAccessContext,
    declared_size: Option<u64>,
}

impl EvidenceUploadRequest {
    pub const fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        access: RequestAccessContext,
        declared_size: Option<u64>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            declared_size,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn declared_size(&self) -> Option<u64> {
        self.declared_size
    }
}

pub trait EvidenceUploadSession: Send {
    fn write_chunk(&mut self, bytes: &[u8]) -> ApplicationResult<()>;

    fn finish(self: Box<Self>) -> ApplicationResult<EvidenceReference>;
}

pub trait EvidenceUploadPort: Send + Sync {
    fn begin_evidence_upload(
        &self,
        request: EvidenceUploadRequest,
    ) -> ApplicationResult<Box<dyn EvidenceUploadSession>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateRecordCommand {
    correlation_id: fasti_domain::RequestCorrelationId,
    access: RequestAccessContext,
    grain: Grain,
}

impl CreateRecordCommand {
    pub const fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        access: RequestAccessContext,
        grain: Grain,
    ) -> Self {
        Self {
            correlation_id,
            access,
            grain,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn grain(&self) -> Grain {
        self.grain
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateRecordOutcome {
    workspace_id: WorkspaceId,
    record_id: RecordId,
    grain: Grain,
}

impl CreateRecordOutcome {
    pub const fn new(workspace_id: WorkspaceId, record_id: RecordId, grain: Grain) -> Self {
        Self {
            workspace_id,
            record_id,
            grain,
        }
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn grain(&self) -> Grain {
        self.grain
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachIdentifierCommand {
    correlation_id: fasti_domain::RequestCorrelationId,
    access: RequestAccessContext,
    record_id: RecordId,
    claim: ExternalIdentifierClaim,
}

impl AttachIdentifierCommand {
    pub const fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        access: RequestAccessContext,
        record_id: RecordId,
        claim: ExternalIdentifierClaim,
    ) -> Self {
        Self {
            correlation_id,
            access,
            record_id,
            claim,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn claim(&self) -> &ExternalIdentifierClaim {
        &self.claim
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachIdentifierOutcome {
    external_identifier_id: ExternalIdentifierId,
    record_id: RecordId,
    created: bool,
}

impl AttachIdentifierOutcome {
    pub const fn new(
        external_identifier_id: ExternalIdentifierId,
        record_id: RecordId,
        created: bool,
    ) -> Self {
        Self {
            external_identifier_id,
            record_id,
            created,
        }
    }

    pub const fn external_identifier_id(&self) -> ExternalIdentifierId {
        self.external_identifier_id
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn created(&self) -> bool {
        self.created
    }
}

pub trait IdentityPort: Send + Sync {
    fn create_record(&self, command: CreateRecordCommand)
        -> ApplicationResult<CreateRecordOutcome>;

    fn attach_identifier(
        &self,
        command: AttachIdentifierCommand,
    ) -> ApplicationResult<AttachIdentifierOutcome>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewItemView {
    review_item_id: ReviewItemId,
    observation_id: fasti_domain::ObservationId,
    current_interpretation_id: InterpretationId,
    status: ReviewStatus,
    candidate_record_ids: Vec<RecordId>,
}

impl ReviewItemView {
    pub fn new(
        review_item_id: ReviewItemId,
        observation_id: fasti_domain::ObservationId,
        current_interpretation_id: InterpretationId,
        status: ReviewStatus,
        candidate_record_ids: Vec<RecordId>,
    ) -> Self {
        Self {
            review_item_id,
            observation_id,
            current_interpretation_id,
            status,
            candidate_record_ids,
        }
    }

    pub const fn review_item_id(&self) -> ReviewItemId {
        self.review_item_id
    }

    pub const fn observation_id(&self) -> fasti_domain::ObservationId {
        self.observation_id
    }

    pub const fn current_interpretation_id(&self) -> InterpretationId {
        self.current_interpretation_id
    }

    pub const fn status(&self) -> ReviewStatus {
        self.status
    }

    pub fn candidate_record_ids(&self) -> &[RecordId] {
        &self.candidate_record_ids
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewQuery {
    correlation_id: fasti_domain::RequestCorrelationId,
    access: RequestAccessContext,
    review_item_id: Option<ReviewItemId>,
}

impl ReviewQuery {
    pub const fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        access: RequestAccessContext,
        review_item_id: Option<ReviewItemId>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            review_item_id,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn review_item_id(&self) -> Option<ReviewItemId> {
        self.review_item_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewAction {
    Defer,
    Resume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewActionCommand {
    correlation_id: fasti_domain::RequestCorrelationId,
    access: RequestAccessContext,
    review_item_id: ReviewItemId,
    action: ReviewAction,
}

impl ReviewActionCommand {
    pub const fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        access: RequestAccessContext,
        review_item_id: ReviewItemId,
        action: ReviewAction,
    ) -> Self {
        Self {
            correlation_id,
            access,
            review_item_id,
            action,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn review_item_id(&self) -> ReviewItemId {
        self.review_item_id
    }

    pub const fn action(&self) -> ReviewAction {
        self.action
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewResolutionTarget {
    Existing(RecordId),
    New(Grain),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveReviewCommand {
    correlation_id: fasti_domain::RequestCorrelationId,
    access: RequestAccessContext,
    review_item_id: ReviewItemId,
    target: ReviewResolutionTarget,
    identifiers: Vec<ExternalIdentifierClaim>,
}

impl ResolveReviewCommand {
    pub fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        access: RequestAccessContext,
        review_item_id: ReviewItemId,
        target: ReviewResolutionTarget,
        identifiers: Vec<ExternalIdentifierClaim>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            review_item_id,
            target,
            identifiers,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn review_item_id(&self) -> ReviewItemId {
        self.review_item_id
    }

    pub const fn target(&self) -> ReviewResolutionTarget {
        self.target
    }

    pub fn identifiers(&self) -> &[ExternalIdentifierClaim] {
        &self.identifiers
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolveReviewOutcome {
    review_item_id: ReviewItemId,
    record_id: RecordId,
    interpretation_id: InterpretationId,
}

impl ResolveReviewOutcome {
    pub const fn new(
        review_item_id: ReviewItemId,
        record_id: RecordId,
        interpretation_id: InterpretationId,
    ) -> Self {
        Self {
            review_item_id,
            record_id,
            interpretation_id,
        }
    }

    pub const fn review_item_id(&self) -> ReviewItemId {
        self.review_item_id
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn interpretation_id(&self) -> InterpretationId {
        self.interpretation_id
    }
}

pub trait ReviewPort: Send + Sync {
    fn inspect_reviews(&self, query: ReviewQuery) -> ApplicationResult<Vec<ReviewItemView>>;

    fn change_review_status(
        &self,
        command: ReviewActionCommand,
    ) -> ApplicationResult<ReviewItemView>;

    fn resolve_review(
        &self,
        command: ResolveReviewCommand,
    ) -> ApplicationResult<ResolveReviewOutcome>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentitySeedEntry {
    key: String,
    grain: Grain,
    identifiers: Vec<ExternalIdentifierClaim>,
}

impl IdentitySeedEntry {
    pub fn new(
        key: impl Into<String>,
        grain: Grain,
        identifiers: Vec<ExternalIdentifierClaim>,
    ) -> Self {
        Self {
            key: key.into(),
            grain,
            identifiers,
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub const fn grain(&self) -> Grain {
        self.grain
    }

    pub fn identifiers(&self) -> &[ExternalIdentifierClaim] {
        &self.identifiers
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentitySeedManifest {
    version: String,
    entries: Vec<IdentitySeedEntry>,
}

impl IdentitySeedManifest {
    pub fn new(version: impl Into<String>, entries: Vec<IdentitySeedEntry>) -> Self {
        Self {
            version: version.into(),
            entries,
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn entries(&self) -> &[IdentitySeedEntry] {
        &self.entries
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyIdentitySeedCommand {
    correlation_id: fasti_domain::RequestCorrelationId,
    access: RequestAccessContext,
    manifest: IdentitySeedManifest,
    dry_run: bool,
}

impl ApplyIdentitySeedCommand {
    pub fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        access: RequestAccessContext,
        manifest: IdentitySeedManifest,
        dry_run: bool,
    ) -> Self {
        Self {
            correlation_id,
            access,
            manifest,
            dry_run,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn manifest(&self) -> &IdentitySeedManifest {
        &self.manifest
    }

    pub const fn dry_run(&self) -> bool {
        self.dry_run
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentitySeedDisposition {
    WouldCreate,
    Created,
    Reused,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentitySeedEntryOutcome {
    key: String,
    disposition: IdentitySeedDisposition,
    record_id: Option<RecordId>,
}

impl IdentitySeedEntryOutcome {
    pub fn new(
        key: impl Into<String>,
        disposition: IdentitySeedDisposition,
        record_id: Option<RecordId>,
    ) -> Self {
        Self {
            key: key.into(),
            disposition,
            record_id,
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub const fn disposition(&self) -> IdentitySeedDisposition {
        self.disposition
    }

    pub const fn record_id(&self) -> Option<RecordId> {
        self.record_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyIdentitySeedOutcome {
    version: String,
    dry_run: bool,
    entries: Vec<IdentitySeedEntryOutcome>,
}

impl ApplyIdentitySeedOutcome {
    pub fn new(
        version: impl Into<String>,
        dry_run: bool,
        entries: Vec<IdentitySeedEntryOutcome>,
    ) -> Self {
        Self {
            version: version.into(),
            dry_run,
            entries,
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub const fn dry_run(&self) -> bool {
        self.dry_run
    }

    pub fn entries(&self) -> &[IdentitySeedEntryOutcome] {
        &self.entries
    }
}

pub trait IdentitySeedPort: Send + Sync {
    fn apply_identity_seed(
        &self,
        command: ApplyIdentitySeedCommand,
    ) -> ApplicationResult<ApplyIdentitySeedOutcome>;
}

pub trait LocalKernel:
    AccessAdministrationPort
    + EvidenceUploadPort
    + IdentityPort
    + IdentitySeedPort
    + crate::ObservationAcceptancePort
    + crate::ReceiptStreamPort
    + ReviewPort
    + Send
    + Sync
{
}

impl<T> LocalKernel for T where
    T: AccessAdministrationPort
        + EvidenceUploadPort
        + IdentityPort
        + IdentitySeedPort
        + crate::ObservationAcceptancePort
        + crate::ReceiptStreamPort
        + ReviewPort
        + Send
        + Sync
{
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_round_trip_is_explicit_and_redacted_by_type() {
        let value = "ab".repeat(32);
        let secret = SecretMaterial::try_from_hex(&value).expect("valid secret");
        assert_eq!(secret.expose_hex(), value);
        assert!(SecretMaterial::try_from_hex(&"AB".repeat(32)).is_err());
    }
}
