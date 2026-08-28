//! Provider metadata application commands.
//!
//! Provider adapters fetch outside the local transaction, then hand validated
//! claims to this port. The provider coordinate remains evidence attached to a
//! Fasti Record; it never becomes the Record identity.

use crate::{ApplicationResult, RequestAccessContext};
use fasti_domain::{
    ExternalIdentifierClaim, FieldClaim, FieldKey, Grain, RecordId, RequestCorrelationId,
};

pub const MAX_PROVIDER_METADATA_FIELDS: usize = 16;

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderMetadataField {
    field_key: FieldKey,
    claim: FieldClaim,
}

impl ProviderMetadataField {
    pub const fn new(field_key: FieldKey, claim: FieldClaim) -> Self {
        Self { field_key, claim }
    }

    pub const fn field_key(&self) -> &FieldKey {
        &self.field_key
    }

    pub const fn claim(&self) -> &FieldClaim {
        &self.claim
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateProviderRecordCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    grain: Grain,
    identifier: ExternalIdentifierClaim,
    fields: Vec<ProviderMetadataField>,
}

impl CreateProviderRecordCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        grain: Grain,
        identifier: ExternalIdentifierClaim,
        fields: Vec<ProviderMetadataField>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            grain,
            identifier,
            fields,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn grain(&self) -> Grain {
        self.grain
    }

    pub const fn identifier(&self) -> &ExternalIdentifierClaim {
        &self.identifier
    }

    pub fn fields(&self) -> &[ProviderMetadataField] {
        &self.fields
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplyProviderMetadataCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    record_id: RecordId,
    identifier: ExternalIdentifierClaim,
    fields: Vec<ProviderMetadataField>,
}

impl ApplyProviderMetadataCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        record_id: RecordId,
        identifier: ExternalIdentifierClaim,
        fields: Vec<ProviderMetadataField>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            record_id,
            identifier,
            fields,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn identifier(&self) -> &ExternalIdentifierClaim {
        &self.identifier
    }

    pub fn fields(&self) -> &[ProviderMetadataField] {
        &self.fields
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateProviderRecordOutcome {
    record_id: RecordId,
    grain: Grain,
}

impl CreateProviderRecordOutcome {
    pub const fn new(record_id: RecordId, grain: Grain) -> Self {
        Self { record_id, grain }
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn grain(&self) -> Grain {
        self.grain
    }
}

pub trait ProviderMetadataPort: Send + Sync {
    fn create_provider_record(
        &self,
        command: CreateProviderRecordCommand,
    ) -> ApplicationResult<CreateProviderRecordOutcome>;

    fn apply_provider_metadata(
        &self,
        command: ApplyProviderMetadataCommand,
    ) -> ApplicationResult<()>;
}
