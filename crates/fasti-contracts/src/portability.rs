//! Internal staged B3 `.fasti` archive-v1 manifest representation.
//!
//! The archive-v1 stream inventory is frozen. This module does not activate an
//! HTTP, CLI, SDK, public registry, or runtime capability.

use fasti_application::{
    ChecksummedWorkspaceManifest, PortabilityLimits, WorkspaceBlobDescriptor,
    WorkspaceExportEntity, WorkspaceManifest, WorkspaceManifestError, WorkspaceStreamDescriptor,
    MAX_PORTABLE_JSON_INTEGER, WORKSPACE_ARCHIVE_FORMAT_VERSION,
};
use fasti_domain::{EvidenceId, Sha256Digest, WorkspaceId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum WorkspaceManifestFormatDto {
    #[serde(rename = "fasti.workspace.manifest")]
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportScopeDto {
    FullWorkspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ArchiveProfileDto {
    #[serde(rename = "zstd-l3-w22")]
    ZstdL3W22,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RestorePolicyDto {
    CleanOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryGrantPolicyDto {
    RequireFreshBootstrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceExportEntityDto {
    Workspaces,
    Profiles,
    Clients,
    Records,
    #[serde(rename = "namespaces")]
    NamespaceDefinitions,
    ExternalIdentifiers,
    Evidence,
    Observations,
    ObservationClues,
    Occurrences,
    Interpretations,
    ReviewItems,
    ReviewCandidates,
    Corrections,
    Receipts,
    Operations,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceStreamDescriptorDto {
    pub entity: WorkspaceExportEntityDto,
    #[schemars(range(min = 0, max = 9007199254740991_i64))]
    pub row_count: u64,
    #[schemars(range(min = 0, max = 9007199254740991_i64))]
    pub byte_length: u64,
    #[schemars(length(equal = 71), regex(pattern = r"^sha256:[0-9a-f]{64}$"), extend("format" = "sha256"))]
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceBlobDescriptorDto {
    #[schemars(length(equal = 36), regex(pattern = r"^evd_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-evidence-id"))]
    pub evidence_id: String,
    #[schemars(range(min = 0, max = 9007199254740991_i64))]
    pub byte_length: u64,
    #[schemars(length(equal = 71), regex(pattern = r"^sha256:[0-9a-f]{64}$"), extend("format" = "sha256"))]
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceManifestDto {
    pub format: WorkspaceManifestFormatDto,
    #[schemars(range(min = 1, max = 1))]
    pub format_version: u32,
    #[schemars(length(equal = 36), regex(pattern = r"^wsp_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-workspace-id"))]
    pub workspace_id: String,
    pub export_scope: ExportScopeDto,
    pub archive_profile: ArchiveProfileDto,
    pub restore_policy: RestorePolicyDto,
    pub recovery_grant_policy: RecoveryGrantPolicyDto,
    #[schemars(range(min = 0, max = 9007199254740991_i64))]
    pub workspace_revision: u64,
    #[schemars(length(min = 1, max = 64))]
    pub contract_version: String,
    #[schemars(range(min = 0, max = 4294967295_i64))]
    pub migration_version: u32,
    #[schemars(length(equal = 71), regex(pattern = r"^sha256:[0-9a-f]{64}$"), extend("format" = "sha256"))]
    pub migration_digest: String,
    #[schemars(length(equal = 16))]
    pub streams: Vec<WorkspaceStreamDescriptorDto>,
    pub blobs: Vec<WorkspaceBlobDescriptorDto>,
}

/// The digest covers RFC 8785 canonical JSON bytes of `manifest` only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChecksummedWorkspaceManifestDto {
    pub manifest: WorkspaceManifestDto,
    #[schemars(length(equal = 71), regex(pattern = r"^sha256:[0-9a-f]{64}$"), extend("format" = "sha256"))]
    pub manifest_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceManifestConversionError {
    UnsupportedFormatVersion,
    InvalidWorkspaceId,
    EmptyContractVersion,
    ContractVersionTooLong,
    PortableIntegerOutOfRange,
    InvalidMigrationDigest,
    StreamCountExceeded,
    StreamRowCountExceeded,
    StreamByteLengthExceeded,
    InvalidStreamDigest,
    BlobCountExceeded,
    DescriptorCountExceeded,
    BlobByteLengthExceeded,
    InvalidEvidenceId,
    InvalidBlobDigest,
    UncompressedBytesExceeded,
    InvalidManifest(WorkspaceManifestError),
    InvalidManifestDigest,
    CanonicalizationFailed,
    ManifestDigestMismatch,
}

impl fmt::Display for WorkspaceManifestConversionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid hostile workspace manifest: {self:?}")
    }
}

impl std::error::Error for WorkspaceManifestConversionError {}

impl From<WorkspaceExportEntityDto> for WorkspaceExportEntity {
    fn from(value: WorkspaceExportEntityDto) -> Self {
        match value {
            WorkspaceExportEntityDto::Workspaces => Self::Workspaces,
            WorkspaceExportEntityDto::Profiles => Self::Profiles,
            WorkspaceExportEntityDto::Clients => Self::Clients,
            WorkspaceExportEntityDto::Records => Self::Records,
            WorkspaceExportEntityDto::NamespaceDefinitions => Self::NamespaceDefinitions,
            WorkspaceExportEntityDto::ExternalIdentifiers => Self::ExternalIdentifiers,
            WorkspaceExportEntityDto::Evidence => Self::Evidence,
            WorkspaceExportEntityDto::Observations => Self::Observations,
            WorkspaceExportEntityDto::ObservationClues => Self::ObservationClues,
            WorkspaceExportEntityDto::Occurrences => Self::Occurrences,
            WorkspaceExportEntityDto::Interpretations => Self::Interpretations,
            WorkspaceExportEntityDto::ReviewItems => Self::ReviewItems,
            WorkspaceExportEntityDto::ReviewCandidates => Self::ReviewCandidates,
            WorkspaceExportEntityDto::Corrections => Self::Corrections,
            WorkspaceExportEntityDto::Receipts => Self::Receipts,
            WorkspaceExportEntityDto::Operations => Self::Operations,
        }
    }
}

impl From<WorkspaceExportEntity> for WorkspaceExportEntityDto {
    fn from(value: WorkspaceExportEntity) -> Self {
        match value {
            WorkspaceExportEntity::Workspaces => Self::Workspaces,
            WorkspaceExportEntity::Profiles => Self::Profiles,
            WorkspaceExportEntity::Clients => Self::Clients,
            WorkspaceExportEntity::Records => Self::Records,
            WorkspaceExportEntity::NamespaceDefinitions => Self::NamespaceDefinitions,
            WorkspaceExportEntity::ExternalIdentifiers => Self::ExternalIdentifiers,
            WorkspaceExportEntity::Evidence => Self::Evidence,
            WorkspaceExportEntity::Observations => Self::Observations,
            WorkspaceExportEntity::ObservationClues => Self::ObservationClues,
            WorkspaceExportEntity::Occurrences => Self::Occurrences,
            WorkspaceExportEntity::Interpretations => Self::Interpretations,
            WorkspaceExportEntity::ReviewItems => Self::ReviewItems,
            WorkspaceExportEntity::ReviewCandidates => Self::ReviewCandidates,
            WorkspaceExportEntity::Corrections => Self::Corrections,
            WorkspaceExportEntity::Receipts => Self::Receipts,
            WorkspaceExportEntity::Operations => Self::Operations,
        }
    }
}

/// One contract-owned outbound projection for archive production.
///
/// It keeps the DTO, final canonical `manifest.json` bytes, and the
/// application value whose digest was verified against the same canonical body
/// together. Store adapters do not rebuild or separately pair these values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalWorkspaceManifestProjection {
    dto: ChecksummedWorkspaceManifestDto,
    canonical_json_bytes: Vec<u8>,
    application: ChecksummedWorkspaceManifest,
}

impl CanonicalWorkspaceManifestProjection {
    /// Project the application manifest into the sole owned archive-v1 wire
    /// model and compute its RFC 8785/JCS body checksum.
    ///
    /// Outer archive adapters use this constructor instead of rebuilding or
    /// separately pairing JSON fields, canonical bytes, and application state.
    pub fn try_from_application(
        manifest: WorkspaceManifest,
    ) -> Result<Self, WorkspaceManifestConversionError> {
        let body = WorkspaceManifestDto {
            format: WorkspaceManifestFormatDto::V1,
            format_version: manifest.format_version(),
            workspace_id: manifest.workspace_id().to_string(),
            export_scope: ExportScopeDto::FullWorkspace,
            archive_profile: ArchiveProfileDto::ZstdL3W22,
            restore_policy: RestorePolicyDto::CleanOnly,
            recovery_grant_policy: RecoveryGrantPolicyDto::RequireFreshBootstrap,
            workspace_revision: manifest.workspace_revision(),
            contract_version: manifest.contract_version().to_owned(),
            migration_version: manifest.migration_version(),
            migration_digest: manifest.migration_digest().as_str().to_owned(),
            streams: manifest
                .streams()
                .iter()
                .map(|stream| WorkspaceStreamDescriptorDto {
                    entity: stream.entity().into(),
                    row_count: stream.row_count(),
                    byte_length: stream.byte_length(),
                    digest: stream.digest().as_str().to_owned(),
                })
                .collect(),
            blobs: manifest
                .blobs()
                .iter()
                .map(|blob| WorkspaceBlobDescriptorDto {
                    evidence_id: blob.evidence_id().to_string(),
                    byte_length: blob.byte_length(),
                    digest: blob.digest().as_str().to_owned(),
                })
                .collect(),
        };
        let canonical_body = serde_json_canonicalizer::to_vec(&body)
            .map_err(|_| WorkspaceManifestConversionError::CanonicalizationFailed)?;
        let manifest_digest =
            Sha256Digest::parse(format!("sha256:{:x}", Sha256::digest(&canonical_body)))
                .expect("SHA-256 output is canonical lowercase hexadecimal");
        let dto = ChecksummedWorkspaceManifestDto {
            manifest: body,
            manifest_digest: manifest_digest.as_str().to_owned(),
        };
        let canonical_json_bytes = serde_json_canonicalizer::to_vec(&dto)
            .map_err(|_| WorkspaceManifestConversionError::CanonicalizationFailed)?;
        let application = ChecksummedWorkspaceManifest::try_from_contract_verified(
            manifest,
            &canonical_body,
            manifest_digest,
        )
        .map_err(|_| WorkspaceManifestConversionError::ManifestDigestMismatch)?;
        Ok(Self {
            dto,
            canonical_json_bytes,
            application,
        })
    }

    pub const fn dto(&self) -> &ChecksummedWorkspaceManifestDto {
        &self.dto
    }

    /// RFC 8785/JCS bytes for the complete checksummed `manifest.json` object.
    pub fn canonical_json_bytes(&self) -> &[u8] {
        &self.canonical_json_bytes
    }

    pub const fn application_manifest(&self) -> &ChecksummedWorkspaceManifest {
        &self.application
    }

    pub fn into_parts(
        self,
    ) -> (
        ChecksummedWorkspaceManifestDto,
        Vec<u8>,
        ChecksummedWorkspaceManifest,
    ) {
        (self.dto, self.canonical_json_bytes, self.application)
    }
}

impl ChecksummedWorkspaceManifestDto {
    /// Convert one hostile archive manifest into the sole application model.
    ///
    /// Restore adapters must call this conversion after their byte-level
    /// archive ceilings and before staging any entry. Schemars annotations are
    /// documentation; every security-relevant bound is enforced again here.
    pub fn try_into_application(
        self,
        limits: PortabilityLimits,
    ) -> Result<ChecksummedWorkspaceManifest, WorkspaceManifestConversionError> {
        let body = &self.manifest;
        if body.format_version != WORKSPACE_ARCHIVE_FORMAT_VERSION {
            return Err(WorkspaceManifestConversionError::UnsupportedFormatVersion);
        }
        let workspace_id = body
            .workspace_id
            .parse::<WorkspaceId>()
            .map_err(|_| WorkspaceManifestConversionError::InvalidWorkspaceId)?;
        let contract_version_length = body.contract_version.chars().count();
        if contract_version_length == 0 {
            return Err(WorkspaceManifestConversionError::EmptyContractVersion);
        }
        if contract_version_length > 64 {
            return Err(WorkspaceManifestConversionError::ContractVersionTooLong);
        }
        if body.workspace_revision > MAX_PORTABLE_JSON_INTEGER {
            return Err(WorkspaceManifestConversionError::PortableIntegerOutOfRange);
        }
        let migration_digest = Sha256Digest::parse(&body.migration_digest)
            .map_err(|_| WorkspaceManifestConversionError::InvalidMigrationDigest)?;

        let max_entries = limits.max_entries.get();
        if u64::try_from(body.streams.len()).map_or(true, |count| count > max_entries) {
            return Err(WorkspaceManifestConversionError::StreamCountExceeded);
        }
        let mut uncompressed_bytes = 0_u64;
        let mut streams = Vec::with_capacity(body.streams.len());
        for stream in &body.streams {
            if stream.row_count > MAX_PORTABLE_JSON_INTEGER
                || stream.byte_length > MAX_PORTABLE_JSON_INTEGER
            {
                return Err(WorkspaceManifestConversionError::PortableIntegerOutOfRange);
            }
            if stream.row_count > limits.max_rows_per_stream.get() {
                return Err(WorkspaceManifestConversionError::StreamRowCountExceeded);
            }
            if stream.byte_length > limits.max_entry_bytes.get() {
                return Err(WorkspaceManifestConversionError::StreamByteLengthExceeded);
            }
            uncompressed_bytes = uncompressed_bytes
                .checked_add(stream.byte_length)
                .ok_or(WorkspaceManifestConversionError::UncompressedBytesExceeded)?;
            let digest = Sha256Digest::parse(&stream.digest)
                .map_err(|_| WorkspaceManifestConversionError::InvalidStreamDigest)?;
            streams.push(WorkspaceStreamDescriptor::new(
                stream.entity.into(),
                stream.row_count,
                stream.byte_length,
                digest,
            ));
        }

        if u64::try_from(body.blobs.len()).map_or(true, |count| count > max_entries) {
            return Err(WorkspaceManifestConversionError::BlobCountExceeded);
        }
        if body
            .streams
            .len()
            .checked_add(body.blobs.len())
            .and_then(|count| count.checked_add(1))
            .and_then(|count| u64::try_from(count).ok())
            .is_none_or(|count| count > max_entries)
        {
            return Err(WorkspaceManifestConversionError::DescriptorCountExceeded);
        }
        let mut blobs = Vec::with_capacity(body.blobs.len());
        for blob in &body.blobs {
            if blob.byte_length > MAX_PORTABLE_JSON_INTEGER {
                return Err(WorkspaceManifestConversionError::PortableIntegerOutOfRange);
            }
            if blob.byte_length > limits.max_entry_bytes.get() {
                return Err(WorkspaceManifestConversionError::BlobByteLengthExceeded);
            }
            uncompressed_bytes = uncompressed_bytes
                .checked_add(blob.byte_length)
                .ok_or(WorkspaceManifestConversionError::UncompressedBytesExceeded)?;
            let evidence_id = blob
                .evidence_id
                .parse::<EvidenceId>()
                .map_err(|_| WorkspaceManifestConversionError::InvalidEvidenceId)?;
            let digest = Sha256Digest::parse(&blob.digest)
                .map_err(|_| WorkspaceManifestConversionError::InvalidBlobDigest)?;
            blobs.push(WorkspaceBlobDescriptor::new(
                evidence_id,
                blob.byte_length,
                digest,
            ));
        }
        if uncompressed_bytes > limits.max_uncompressed_bytes.get() {
            return Err(WorkspaceManifestConversionError::UncompressedBytesExceeded);
        }

        let manifest = WorkspaceManifest::try_new(
            workspace_id,
            body.workspace_revision,
            body.contract_version.clone(),
            body.migration_version,
            migration_digest,
            streams,
            blobs,
        )
        .map_err(WorkspaceManifestConversionError::InvalidManifest)?;

        let supplied_manifest_digest = Sha256Digest::parse(&self.manifest_digest)
            .map_err(|_| WorkspaceManifestConversionError::InvalidManifestDigest)?;
        let canonical_body = serde_json_canonicalizer::to_vec(body)
            .map_err(|_| WorkspaceManifestConversionError::CanonicalizationFailed)?;
        ChecksummedWorkspaceManifest::try_from_contract_verified(
            manifest,
            &canonical_body,
            supplied_manifest_digest,
        )
        .map_err(|_| WorkspaceManifestConversionError::ManifestDigestMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::generate::SchemaSettings;
    use std::num::NonZeroU64;

    fn checked_example() -> ChecksummedWorkspaceManifestDto {
        serde_json::from_str(include_str!(
            "../../../contracts/portability/v1/workspace-manifest.example.json"
        ))
        .expect("checked-in portability example")
    }

    fn limits() -> PortabilityLimits {
        let bytes = NonZeroU64::new(1_000_000).expect("non-zero byte limit");
        let entries = NonZeroU64::new(64).expect("non-zero entry limit");
        let one = NonZeroU64::new(1).expect("non-zero unit limit");
        PortabilityLimits {
            max_snapshot_bytes: bytes,
            max_wal_growth_bytes: bytes,
            max_archive_bytes: bytes,
            max_uncompressed_bytes: bytes,
            max_entry_bytes: bytes,
            max_entries: entries,
            max_rows_per_stream: entries,
            max_path_bytes: bytes,
            max_path_depth: entries,
            max_decompression_ratio: entries,
            scratch_ceiling_bytes: bytes,
            cleanup_reserve_bytes: bytes,
            backup_step_pages: one,
            backup_step_millis: one,
        }
    }

    #[test]
    fn checked_in_manifest_example_matches_the_frozen_internal_archive_v1_dto() {
        let manifest = checked_example();

        assert_eq!(
            manifest.manifest.streams.len(),
            WorkspaceExportEntity::ALL.len()
        );
        assert_eq!(
            manifest
                .manifest
                .streams
                .iter()
                .map(|stream| stream.entity)
                .collect::<Vec<_>>(),
            vec![
                WorkspaceExportEntityDto::Workspaces,
                WorkspaceExportEntityDto::Profiles,
                WorkspaceExportEntityDto::Clients,
                WorkspaceExportEntityDto::Records,
                WorkspaceExportEntityDto::NamespaceDefinitions,
                WorkspaceExportEntityDto::ExternalIdentifiers,
                WorkspaceExportEntityDto::Evidence,
                WorkspaceExportEntityDto::Observations,
                WorkspaceExportEntityDto::ObservationClues,
                WorkspaceExportEntityDto::Occurrences,
                WorkspaceExportEntityDto::Interpretations,
                WorkspaceExportEntityDto::ReviewItems,
                WorkspaceExportEntityDto::ReviewCandidates,
                WorkspaceExportEntityDto::Corrections,
                WorkspaceExportEntityDto::Receipts,
                WorkspaceExportEntityDto::Operations,
            ]
        );
        assert_eq!(manifest.manifest.format_version, 1);
        assert_eq!(manifest.manifest.workspace_revision, 1);
        assert_eq!(
            manifest.manifest.archive_profile,
            ArchiveProfileDto::ZstdL3W22
        );
        assert_eq!(
            manifest.manifest.recovery_grant_policy,
            RecoveryGrantPolicyDto::RequireFreshBootstrap
        );
        let converted = manifest
            .try_into_application(limits())
            .expect("strict hostile-boundary conversion");
        assert_eq!(converted.manifest().workspace_revision(), 1);
        assert_eq!(
            converted.digest().as_str(),
            "sha256:caf6143ace824b10a83f4d4d9ad0a72f65479840f657e0094e6ef8b3415fbd72"
        );
    }

    #[test]
    fn checked_in_manifest_schema_and_dto_share_the_bounded_contract() {
        let checked: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/portability/v1/workspace-manifest.schema.json"
        ))
        .expect("checked-in portability schema");
        assert_eq!(
            checked.get("$schema").and_then(serde_json::Value::as_str),
            Some("https://json-schema.org/draft/2020-12/schema")
        );
        assert_eq!(
            checked
                .get("x-fasti-contract-state")
                .and_then(serde_json::Value::as_str),
            Some("internal_staged_archive_v1")
        );
        let current_stream_count = serde_json::json!(WorkspaceExportEntity::ALL.len());
        assert_eq!(
            checked.pointer("/$defs/WorkspaceManifest/properties/streams/minItems"),
            Some(&current_stream_count)
        );
        assert_eq!(
            checked.pointer("/$defs/WorkspaceManifest/properties/streams/maxItems"),
            Some(&current_stream_count)
        );
        assert_eq!(
            checked
                .pointer("/$defs/WorkspaceManifest/properties/streams/prefixItems")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(WorkspaceExportEntity::ALL.len())
        );
        assert_eq!(
            checked.pointer(
                "/$defs/WorkspaceManifest/properties/streams/prefixItems/0/allOf/1/properties/entity/const"
            ),
            Some(&serde_json::json!("workspaces"))
        );
        assert_eq!(
            checked.pointer(
                "/$defs/WorkspaceManifest/properties/streams/prefixItems/4/allOf/1/properties/entity/const"
            ),
            Some(&serde_json::json!("namespaces"))
        );
        assert_eq!(
            checked.pointer(
                "/$defs/WorkspaceManifest/properties/streams/prefixItems/5/allOf/1/properties/entity/const"
            ),
            Some(&serde_json::json!("external_identifiers"))
        );
        assert_eq!(
            checked.pointer(&format!(
                "/$defs/WorkspaceManifest/properties/streams/prefixItems/{}/allOf/1/properties/entity/const",
                WorkspaceExportEntity::ALL.len() - 1
            )),
            Some(&serde_json::json!("operations"))
        );
        assert_eq!(
            checked.pointer("/$defs/WorkspaceManifest/properties/workspace_revision/minimum"),
            Some(&serde_json::json!(0))
        );
        assert_eq!(
            checked.pointer("/$defs/WorkspaceManifest/properties/workspace_revision/maximum"),
            Some(&serde_json::json!(MAX_PORTABLE_JSON_INTEGER))
        );
        assert_eq!(
            checked.pointer("/$defs/WorkspaceManifest/properties/contract_version/maxLength"),
            Some(&serde_json::json!(64))
        );
        assert_eq!(
            checked.pointer("/$defs/WorkspaceManifest/properties/migration_version/maximum"),
            Some(&serde_json::json!(u32::MAX))
        );

        let generated = SchemaSettings::draft2020_12()
            .into_generator()
            .into_root_schema_for::<ChecksummedWorkspaceManifestDto>();
        let generated = serde_json::to_value(generated).expect("generated manifest schema");
        assert_eq!(
            generated.pointer("/$defs/WorkspaceExportEntityDto/enum/4"),
            Some(&serde_json::json!("namespaces"))
        );
        assert_eq!(
            generated.pointer("/$defs/WorkspaceExportEntityDto/enum/5"),
            Some(&serde_json::json!("external_identifiers"))
        );
        assert_eq!(
            generated.pointer("/$defs/WorkspaceManifestDto/properties/streams/minItems"),
            Some(&current_stream_count)
        );
        assert_eq!(
            generated.pointer("/$defs/WorkspaceManifestDto/properties/streams/maxItems"),
            Some(&current_stream_count)
        );
        assert_eq!(
            generated.pointer("/$defs/WorkspaceManifestDto/properties/workspace_revision/minimum"),
            Some(&serde_json::json!(0))
        );
        assert_eq!(
            generated.pointer("/$defs/WorkspaceManifestDto/properties/workspace_revision/maximum"),
            Some(&serde_json::json!(MAX_PORTABLE_JSON_INTEGER))
        );
        assert_eq!(
            generated.pointer("/$defs/WorkspaceManifestDto/properties/contract_version/maxLength"),
            Some(&serde_json::json!(64))
        );
        assert_eq!(
            generated.pointer("/$defs/WorkspaceManifestDto/properties/migration_version/maximum"),
            Some(&serde_json::json!(u32::MAX))
        );
    }

    #[test]
    fn application_manifest_has_one_checksummed_canonical_wire_projection() {
        let expected = checked_example();
        let application = expected
            .clone()
            .try_into_application(limits())
            .expect("checked example converts to the application manifest");

        let projected = CanonicalWorkspaceManifestProjection::try_from_application(
            application.manifest().clone(),
        )
        .expect("application manifest projects to archive v1");
        assert_eq!(projected.dto(), &expected);
        assert_eq!(projected.application_manifest(), &application);

        let decoded: ChecksummedWorkspaceManifestDto =
            serde_json::from_slice(projected.canonical_json_bytes())
                .expect("canonical bytes decode");
        assert_eq!(&decoded, projected.dto());
        assert_eq!(
            decoded.clone().try_into_application(limits()),
            Ok(application),
            "the owned wire projection must round-trip through the hostile boundary"
        );

        let mut mutated = decoded;
        mutated.manifest.workspace_revision += 1;
        assert_eq!(
            mutated.try_into_application(limits()),
            Err(WorkspaceManifestConversionError::ManifestDigestMismatch),
            "outbound wire mutations cannot retain the canonical projection digest"
        );
    }

    #[test]
    fn manifest_dto_rejects_unknown_fields() {
        let mut value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/portability/v1/workspace-manifest.example.json"
        ))
        .expect("checked-in portability example");
        value["manifest"]["credentials"] = serde_json::json!([]);
        assert!(serde_json::from_value::<ChecksummedWorkspaceManifestDto>(value).is_err());
    }

    #[test]
    fn manifest_dto_rejects_migration_versions_above_u32() {
        let mut value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/portability/v1/workspace-manifest.example.json"
        ))
        .expect("checked-in portability example");
        value["manifest"]["migration_version"] = serde_json::json!(4_294_967_296_u64);
        assert!(serde_json::from_value::<ChecksummedWorkspaceManifestDto>(value).is_err());
    }

    #[test]
    fn hostile_conversion_rejects_version_ids_digests_bounds_and_stream_order() {
        let mut value = checked_example();
        value.manifest.format_version = 2;
        assert_eq!(
            value.try_into_application(limits()),
            Err(WorkspaceManifestConversionError::UnsupportedFormatVersion)
        );

        let mut value = checked_example();
        value.manifest.workspace_id = "wsp_not-a-v7-id".to_owned();
        assert_eq!(
            value.try_into_application(limits()),
            Err(WorkspaceManifestConversionError::InvalidWorkspaceId)
        );

        let mut value = checked_example();
        value.manifest.streams[0].digest = format!("sha256:{}", "AB".repeat(32));
        assert_eq!(
            value.try_into_application(limits()),
            Err(WorkspaceManifestConversionError::InvalidStreamDigest)
        );

        let mut value = checked_example();
        value.manifest.contract_version = "x".repeat(65);
        assert_eq!(
            value.try_into_application(limits()),
            Err(WorkspaceManifestConversionError::ContractVersionTooLong)
        );

        let mut value = checked_example();
        value.manifest.streams[0].row_count = 65;
        assert_eq!(
            value.try_into_application(limits()),
            Err(WorkspaceManifestConversionError::StreamRowCountExceeded)
        );

        let value = checked_example();
        let mut descriptor_only_limit = limits();
        descriptor_only_limit.max_entries = NonZeroU64::new(
            u64::try_from(value.manifest.streams.len() + value.manifest.blobs.len())
                .expect("test descriptor count fits u64"),
        )
        .expect("example contains descriptors");
        assert_eq!(
            value.try_into_application(descriptor_only_limit),
            Err(WorkspaceManifestConversionError::DescriptorCountExceeded),
            "the mandatory final manifest.json must consume one archive entry"
        );

        let mut value = checked_example();
        value.manifest.streams.swap(0, 1);
        assert_eq!(
            value.try_into_application(limits()),
            Err(WorkspaceManifestConversionError::InvalidManifest(
                WorkspaceManifestError::IncompleteStreamSet
            ))
        );
    }

    #[test]
    fn hostile_conversion_recomputes_the_rfc_8785_manifest_digest() {
        let mut value = checked_example();
        value.manifest.workspace_revision += 1;
        assert_eq!(
            value.try_into_application(limits()),
            Err(WorkspaceManifestConversionError::ManifestDigestMismatch)
        );
    }
}
