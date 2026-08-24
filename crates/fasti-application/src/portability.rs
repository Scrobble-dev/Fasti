//! B3 workspace portability capabilities and adapter ports.
//!
//! Verification and export report bounded summary counts. Neither exposes
//! SQLite, filesystem, transport, provider, or UI details to callers.
//!
//! Export writes to a caller-supplied [`std::io::Write`] sink so the adapter
//! can stream bounded pages instead of materializing a workspace in memory.
//! `std::io::Write` is a standard-library boundary, not an adapter type, so
//! the domain-inward dependency rule holds.

use crate::{ApplicationResult, RequestAccessContext};
use fasti_domain::{RequestCorrelationId, WorkspaceId};
use std::io::Write;

/// Archive format version written by the export adapter.
///
/// A restore implementation must reject any version it does not understand
/// rather than guessing at the framing.
pub const WORKSPACE_EXPORT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyWorkspaceQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
}

impl VerifyWorkspaceQuery {
    pub const fn new(correlation_id: RequestCorrelationId, access: RequestAccessContext) -> Self {
        Self {
            correlation_id,
            access,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceVerificationOutcome {
    workspace_id: WorkspaceId,
    observations_verified: u64,
    evidence_verified: u64,
    corrections_verified: u64,
}

impl WorkspaceVerificationOutcome {
    pub const fn new(
        workspace_id: WorkspaceId,
        observations_verified: u64,
        evidence_verified: u64,
        corrections_verified: u64,
    ) -> Self {
        Self {
            workspace_id,
            observations_verified,
            evidence_verified,
            corrections_verified,
        }
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn observations_verified(&self) -> u64 {
        self.observations_verified
    }

    pub const fn evidence_verified(&self) -> u64 {
        self.evidence_verified
    }

    pub const fn corrections_verified(&self) -> u64 {
        self.corrections_verified
    }
}

/// Read-only B3 integrity-verification boundary.
///
/// Implementations must re-authorize against current durable state and verify
/// persisted Chronicle relations and evidence bytes before returning success.
pub trait WorkspaceVerificationPort: Send + Sync {
    fn verify_workspace(
        &self,
        query: VerifyWorkspaceQuery,
    ) -> ApplicationResult<WorkspaceVerificationOutcome>;
}

/// One durable entity stream in a workspace export archive.
///
/// The order of [`WorkspaceExportEntity::ALL`] is the order sections are
/// written. It is part of the archive format: changing it changes the bytes
/// and therefore requires a format-version change.
///
/// Excluded by policy, and deliberately absent from this enum: credential
/// secrets, initialization proof material, active authorization grants and
/// their scopes, and node-local listener configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorkspaceExportEntity {
    Workspaces,
    Profiles,
    Clients,
    Records,
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

impl WorkspaceExportEntity {
    /// Every exported entity, in archive section order.
    pub const ALL: [Self; 16] = [
        Self::Workspaces,
        Self::Profiles,
        Self::Clients,
        Self::Records,
        Self::NamespaceDefinitions,
        Self::ExternalIdentifiers,
        Self::Evidence,
        Self::Observations,
        Self::ObservationClues,
        Self::Occurrences,
        Self::Interpretations,
        Self::ReviewItems,
        Self::ReviewCandidates,
        Self::Corrections,
        Self::Receipts,
        Self::Operations,
    ];

    /// Stable section name written into the archive and the manifest.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Workspaces => "workspaces",
            Self::Profiles => "profiles",
            Self::Clients => "clients",
            Self::Records => "records",
            Self::NamespaceDefinitions => "namespaces",
            Self::ExternalIdentifiers => "external_identifiers",
            Self::Evidence => "evidence",
            Self::Observations => "observations",
            Self::ObservationClues => "observation_clues",
            Self::Occurrences => "occurrences",
            Self::Interpretations => "interpretations",
            Self::ReviewItems => "review_items",
            Self::ReviewCandidates => "review_candidates",
            Self::Corrections => "corrections",
            Self::Receipts => "receipts",
            Self::Operations => "operations",
        }
    }

    /// Position of this entity in [`WorkspaceExportEntity::ALL`].
    pub const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportWorkspaceQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
}

impl ExportWorkspaceQuery {
    pub const fn new(correlation_id: RequestCorrelationId, access: RequestAccessContext) -> Self {
        Self {
            correlation_id,
            access,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
}

/// Bounded summary of one completed workspace export.
///
/// Counts are per entity so a restore can assert that it consumed exactly
/// what the export produced. The digest covers every byte written to the
/// sink, so it cannot be embedded in the archive itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceExportOutcome {
    workspace_id: WorkspaceId,
    format_version: u32,
    counts: [u64; WorkspaceExportEntity::ALL.len()],
    bytes_written: u64,
    archive_digest: String,
}

impl WorkspaceExportOutcome {
    pub const fn new(
        workspace_id: WorkspaceId,
        format_version: u32,
        counts: [u64; WorkspaceExportEntity::ALL.len()],
        bytes_written: u64,
        archive_digest: String,
    ) -> Self {
        Self {
            workspace_id,
            format_version,
            counts,
            bytes_written,
            archive_digest,
        }
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn format_version(&self) -> u32 {
        self.format_version
    }

    /// Rows written for one entity section.
    pub const fn count(&self, entity: WorkspaceExportEntity) -> u64 {
        self.counts[entity.index()]
    }

    /// Every per-entity count in archive section order.
    pub fn counts(&self) -> impl Iterator<Item = (WorkspaceExportEntity, u64)> + '_ {
        WorkspaceExportEntity::ALL
            .into_iter()
            .map(|entity| (entity, self.counts[entity.index()]))
    }

    pub const fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// `sha256:<64 lowercase hex>` over every byte written to the sink.
    pub fn archive_digest(&self) -> &str {
        &self.archive_digest
    }
}

/// Read-only B3 workspace export boundary.
///
/// Implementations must:
///
/// - re-authorize against current durable state for every bounded page, so a
///   revocation part-way through a long export stops further disclosure;
/// - write deterministically, so the same durable state produces identical
///   bytes across processes and hosts;
/// - stream bounded pages rather than materializing the workspace;
/// - exclude credential secrets, initialization proof material, and active
///   authorization bindings.
///
/// A partially written sink is not a valid archive. Callers must treat any
/// error as "discard the destination", because bytes already handed to the
/// sink cannot be recalled.
pub trait WorkspaceExportPort: Send + Sync {
    fn export_workspace(
        &self,
        query: ExportWorkspaceQuery,
        sink: &mut dyn Write,
    ) -> ApplicationResult<WorkspaceExportOutcome>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use fasti_domain::{ClientId, CredentialId, ProfileGrantId, ProfileId};

    #[test]
    fn verification_query_derives_workspace_from_access_context() {
        let workspace_id = WorkspaceId::new_v7();
        let access = RequestAccessContext::new(
            workspace_id,
            ProfileId::new_v7(),
            ClientId::new_v7(),
            CredentialId::new_v7(),
            ProfileGrantId::new_v7(),
            1,
        );
        let query = VerifyWorkspaceQuery::new(RequestCorrelationId::new_v7(), access);

        assert_eq!(query.access().workspace_id(), workspace_id);
    }

    #[test]
    fn export_entity_index_matches_declared_section_order() {
        // `index()` is `self as usize`, so a reordered or partially updated
        // `ALL` would silently attribute counts to the wrong entity. Nothing
        // else in the archive would look wrong.
        for (position, entity) in WorkspaceExportEntity::ALL.into_iter().enumerate() {
            assert_eq!(entity.index(), position, "{entity:?} index drifted");
        }
    }

    #[test]
    fn export_entity_section_names_are_unique_and_stable() {
        let mut names: Vec<&str> = WorkspaceExportEntity::ALL
            .into_iter()
            .map(WorkspaceExportEntity::as_str)
            .collect();
        let declared = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), declared, "duplicate export section name");
        assert!(names.iter().all(|name| !name.is_empty()));
    }

    #[test]
    fn export_entity_excludes_secret_and_authorization_bindings() {
        // These tables exist in the store and must never gain a section.
        for forbidden in [
            "credentials",
            "profile_grants",
            "grant_scopes",
            "node_state",
            "listener_configuration",
        ] {
            assert!(
                !WorkspaceExportEntity::ALL
                    .into_iter()
                    .any(|entity| entity.as_str() == forbidden),
                "{forbidden} must not be exported"
            );
        }
    }

    #[test]
    fn export_outcome_reports_counts_in_section_order() {
        let mut counts = [0u64; WorkspaceExportEntity::ALL.len()];
        counts[WorkspaceExportEntity::Observations.index()] = 7;
        counts[WorkspaceExportEntity::Corrections.index()] = 3;
        let outcome = WorkspaceExportOutcome::new(
            WorkspaceId::new_v7(),
            WORKSPACE_EXPORT_FORMAT_VERSION,
            counts,
            2048,
            "sha256:00".to_owned(),
        );

        assert_eq!(outcome.count(WorkspaceExportEntity::Observations), 7);
        assert_eq!(outcome.count(WorkspaceExportEntity::Corrections), 3);
        assert_eq!(outcome.count(WorkspaceExportEntity::Receipts), 0);
        assert_eq!(outcome.bytes_written(), 2048);
        let reported: Vec<_> = outcome.counts().map(|(entity, _)| entity).collect();
        assert_eq!(reported, WorkspaceExportEntity::ALL.to_vec());
    }
}
