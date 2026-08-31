use crate::{
    ClientId, EvidenceId, ExternalIdentifierClaim, ExternalIdentifierId, IdentityAssertionId,
    ReceivedAt, RecordId, Sha256Digest, WorkspaceId,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

pub const MAX_IDENTITY_ASSERTION_EVIDENCE: usize = 16;
pub const MAX_IDENTITY_COVERAGE_SEGMENTS: usize = 64;
pub const MAX_IDENTITY_EPISODE_LINKS: usize = 64;
pub const MAX_IDENTITY_EPISODES_PER_LINK_SIDE: usize = 16;

const MAX_PROVENANCE_TEXT_BYTES: usize = 2_048;
const MAX_IDENTITY_SOURCE_BYTES: usize = 256;
const MAX_IDENTITY_REASONING_BYTES: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityAssertionRelation {
    Exact,
    SubsetOf,
    SupersetOf,
    Overlaps,
    AlternateCutOf,
    Related,
    NotSameAs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityAssertionEvidenceClass {
    Asserted,
    Verified,
    Corroborated,
    Inferred,
    Candidate,
    Disputed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityAssertionStatus {
    Candidate,
    Accepted,
    Disputed,
    Rejected,
    Revoked,
}

impl IdentityAssertionStatus {
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (
                Self::Candidate,
                Self::Accepted | Self::Disputed | Self::Rejected
            ) | (Self::Accepted, Self::Disputed | Self::Revoked)
                | (
                    Self::Disputed,
                    Self::Accepted | Self::Rejected | Self::Revoked
                )
        )
    }

    pub const fn can_route(self) -> bool {
        matches!(self, Self::Accepted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityCoverageMode {
    Single,
    Flat,
    Season,
    Absolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityNumberingSpace {
    Regular,
    Special,
    Trailer,
    Credit,
    Parody,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityOrdering {
    Broadcast,
    Chronological,
    Intended,
    Provider,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdentityAssertionError {
    #[error("identity assertion source and target must be different coordinates")]
    SameCoordinate,
    #[error("identity assertion coverage is missing, unbounded, or invalid")]
    InvalidCoverage,
    #[error("identity assertion episode links are unbounded or invalid")]
    InvalidEpisodeLinks,
    #[error("identity assertion evidence is missing, unbounded, or invalid")]
    InvalidEvidence,
    #[error("identity assertion provenance is missing or unbounded")]
    InvalidProvenance,
    #[error("identity assertion relation requires explicit reasoning")]
    MissingReasoning,
    #[error("identity assertion evidence class does not support its status")]
    InvalidStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityCoverageSegment {
    mode: IdentityCoverageMode,
    season: Option<u32>,
    numbering_space: IdentityNumberingSpace,
    ordering: IdentityOrdering,
    source_start: u32,
    source_end: u32,
    offset: i32,
    region: Option<String>,
}

impl IdentityCoverageSegment {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        mode: IdentityCoverageMode,
        season: Option<u32>,
        numbering_space: IdentityNumberingSpace,
        ordering: IdentityOrdering,
        source_start: u32,
        source_end: u32,
        offset: i32,
        region: Option<String>,
    ) -> Result<Self, IdentityAssertionError> {
        let target_start = i64::from(source_start) + i64::from(offset);
        let target_end = i64::from(source_end) + i64::from(offset);
        let region_is_valid = region.as_deref().is_none_or(|value| {
            value == "*"
                || (value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_uppercase()))
        });
        if source_start == 0
            || source_start > source_end
            || target_start < 1
            || target_end < target_start
            || u32::try_from(target_start).is_err()
            || u32::try_from(target_end).is_err()
            || matches!(mode, IdentityCoverageMode::Season) != season.is_some()
            || !region_is_valid
        {
            return Err(IdentityAssertionError::InvalidCoverage);
        }
        Ok(Self {
            mode,
            season,
            numbering_space,
            ordering,
            source_start,
            source_end,
            offset,
            region,
        })
    }

    pub const fn mode(&self) -> IdentityCoverageMode {
        self.mode
    }

    pub const fn season(&self) -> Option<u32> {
        self.season
    }

    pub const fn numbering_space(&self) -> IdentityNumberingSpace {
        self.numbering_space
    }

    pub const fn ordering(&self) -> IdentityOrdering {
        self.ordering
    }

    pub const fn source_start(&self) -> u32 {
        self.source_start
    }

    pub const fn source_end(&self) -> u32 {
        self.source_end
    }

    pub fn target_start(&self) -> u32 {
        u32::try_from(i64::from(self.source_start) + i64::from(self.offset))
            .expect("validated coverage target start")
    }

    pub fn target_end(&self) -> u32 {
        u32::try_from(i64::from(self.source_end) + i64::from(self.offset))
            .expect("validated coverage target end")
    }

    pub const fn offset(&self) -> i32 {
        self.offset
    }

    pub fn region(&self) -> Option<&str> {
        self.region.as_deref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityEpisodeLinkKind {
    Exact,
    Expands,
    Merges,
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityEpisodeLink {
    from: Vec<u32>,
    to: Vec<u32>,
    kind: IdentityEpisodeLinkKind,
    reason: Option<String>,
}

impl IdentityEpisodeLink {
    pub fn try_new(
        mut from: Vec<u32>,
        mut to: Vec<u32>,
        kind: IdentityEpisodeLinkKind,
        reason: Option<String>,
    ) -> Result<Self, IdentityAssertionError> {
        if from.len() > MAX_IDENTITY_EPISODES_PER_LINK_SIDE
            || to.len() > MAX_IDENTITY_EPISODES_PER_LINK_SIDE
        {
            return Err(IdentityAssertionError::InvalidEpisodeLinks);
        }
        from.sort_unstable();
        from.dedup();
        to.sort_unstable();
        to.dedup();
        let reason_is_valid = reason
            .as_deref()
            .is_none_or(|value| valid_text(value, MAX_PROVENANCE_TEXT_BYTES));
        let shape_is_valid = match kind {
            IdentityEpisodeLinkKind::Exact => from.len() == 1 && to.len() == 1,
            IdentityEpisodeLinkKind::Expands => from.len() == 1 && to.len() >= 2,
            IdentityEpisodeLinkKind::Merges => from.len() >= 2 && to.len() == 1,
            IdentityEpisodeLinkKind::Absent => to.is_empty() && reason.is_some(),
        };
        if from.is_empty()
            || from.contains(&0)
            || to.contains(&0)
            || !reason_is_valid
            || !shape_is_valid
        {
            return Err(IdentityAssertionError::InvalidEpisodeLinks);
        }
        Ok(Self {
            from,
            to,
            kind,
            reason,
        })
    }

    pub fn from(&self) -> &[u32] {
        &self.from
    }

    pub fn to(&self) -> &[u32] {
        &self.to
    }

    pub const fn kind(&self) -> IdentityEpisodeLinkKind {
        self.kind
    }

    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityEvidenceMethod {
    RightsholderAsserted,
    HumanVerified,
    UpstreamDeclared,
    DerivedAirDates,
    HeuristicTitleMatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityAssertionEvidence {
    method: IdentityEvidenceMethod,
    observed_source: String,
    derivation_root: Option<String>,
    reviewer: Option<String>,
    observed_at: NaiveDate,
    evidence_id: Option<EvidenceId>,
}

impl IdentityAssertionEvidence {
    pub fn try_new(
        method: IdentityEvidenceMethod,
        observed_source: impl Into<String>,
        derivation_root: Option<String>,
        reviewer: Option<String>,
        observed_at: NaiveDate,
        evidence_id: Option<EvidenceId>,
    ) -> Result<Self, IdentityAssertionError> {
        let observed_source = observed_source.into();
        let derivation_root_is_valid = derivation_root
            .as_deref()
            .is_none_or(|value| valid_provenance_text(value, MAX_IDENTITY_SOURCE_BYTES));
        let reviewer_is_valid = reviewer
            .as_deref()
            .is_none_or(|value| valid_text(value, MAX_IDENTITY_SOURCE_BYTES));
        if !valid_provenance_text(&observed_source, MAX_PROVENANCE_TEXT_BYTES)
            || !derivation_root_is_valid
            || !reviewer_is_valid
        {
            return Err(IdentityAssertionError::InvalidEvidence);
        }
        Ok(Self {
            method,
            observed_source,
            derivation_root,
            reviewer,
            observed_at,
            evidence_id,
        })
    }

    pub const fn method(&self) -> IdentityEvidenceMethod {
        self.method
    }

    pub fn observed_source(&self) -> &str {
        &self.observed_source
    }

    pub fn derivation_root(&self) -> Option<&str> {
        self.derivation_root.as_deref()
    }

    pub fn reviewer(&self) -> Option<&str> {
        self.reviewer.as_deref()
    }

    pub const fn observed_at(&self) -> NaiveDate {
        self.observed_at
    }

    pub const fn evidence_id(&self) -> Option<EvidenceId> {
        self.evidence_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityAssertion {
    assertion_id: IdentityAssertionId,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    source_external_identifier_id: ExternalIdentifierId,
    target: ExternalIdentifierClaim,
    relation: IdentityAssertionRelation,
    coverage: Vec<IdentityCoverageSegment>,
    episode_links: Vec<IdentityEpisodeLink>,
    evidence_class: IdentityAssertionEvidenceClass,
    evidence: Vec<IdentityAssertionEvidence>,
    id_source: String,
    source_version: Option<String>,
    authority: Option<String>,
    reasoning: Option<String>,
    initial_status: IdentityAssertionStatus,
    created_at: DateTime<Utc>,
}

impl IdentityAssertion {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        assertion_id: IdentityAssertionId,
        workspace_id: WorkspaceId,
        record_id: RecordId,
        source_external_identifier_id: ExternalIdentifierId,
        source: &ExternalIdentifierClaim,
        target: ExternalIdentifierClaim,
        relation: IdentityAssertionRelation,
        coverage: Vec<IdentityCoverageSegment>,
        episode_links: Vec<IdentityEpisodeLink>,
        evidence_class: IdentityAssertionEvidenceClass,
        evidence: Vec<IdentityAssertionEvidence>,
        id_source: impl Into<String>,
        source_version: Option<String>,
        authority: Option<String>,
        reasoning: Option<String>,
        initial_status: IdentityAssertionStatus,
        created_at: ReceivedAt,
    ) -> Result<Self, IdentityAssertionError> {
        let id_source = id_source.into();
        let relation_needs_coverage = matches!(
            relation,
            IdentityAssertionRelation::SubsetOf
                | IdentityAssertionRelation::SupersetOf
                | IdentityAssertionRelation::Overlaps
        );
        if coverage.len() > MAX_IDENTITY_COVERAGE_SEGMENTS
            || (relation_needs_coverage && coverage.is_empty())
        {
            return Err(IdentityAssertionError::InvalidCoverage);
        }
        if episode_links.len() > MAX_IDENTITY_EPISODE_LINKS {
            return Err(IdentityAssertionError::InvalidEpisodeLinks);
        }
        if evidence.is_empty() || evidence.len() > MAX_IDENTITY_ASSERTION_EVIDENCE {
            return Err(IdentityAssertionError::InvalidEvidence);
        }
        let evidence_is_unique = evidence
            .iter()
            .map(|item| (item.method(), item.observed_source()))
            .collect::<HashSet<_>>()
            .len()
            == evidence.len();
        let derivation_roots = evidence
            .iter()
            .filter_map(|item| item.derivation_root())
            .collect::<HashSet<_>>();
        let source_version_is_valid = source_version
            .as_deref()
            .is_none_or(|value| valid_text(value, MAX_IDENTITY_SOURCE_BYTES));
        let authority_is_valid = authority
            .as_deref()
            .is_none_or(|value| valid_text(value, MAX_IDENTITY_SOURCE_BYTES));
        let reasoning_is_valid = reasoning
            .as_deref()
            .is_none_or(|value| valid_text(value, MAX_IDENTITY_REASONING_BYTES));
        if source == &target {
            return Err(IdentityAssertionError::SameCoordinate);
        }
        if !evidence_is_unique {
            return Err(IdentityAssertionError::InvalidEvidence);
        }
        if !valid_provenance_text(&id_source, MAX_IDENTITY_SOURCE_BYTES)
            || !source_version_is_valid
            || !authority_is_valid
        {
            return Err(IdentityAssertionError::InvalidProvenance);
        }
        if !reasoning_is_valid
            || (matches!(relation, IdentityAssertionRelation::NotSameAs)
                && reasoning
                    .as_deref()
                    .is_none_or(|value| value.chars().count() < 20))
        {
            return Err(IdentityAssertionError::MissingReasoning);
        }
        if (matches!(evidence_class, IdentityAssertionEvidenceClass::Asserted)
            && (authority.is_none()
                || !evidence
                    .iter()
                    .any(|item| item.method() == IdentityEvidenceMethod::RightsholderAsserted)))
            || (matches!(evidence_class, IdentityAssertionEvidenceClass::Verified)
                && !evidence.iter().any(|item| {
                    item.method() == IdentityEvidenceMethod::HumanVerified
                        && item.reviewer().is_some()
                }))
            || (matches!(evidence_class, IdentityAssertionEvidenceClass::Corroborated)
                && (evidence.len() < 2 || derivation_roots.len() != evidence.len()))
            || (initial_status.can_route()
                && matches!(
                    evidence_class,
                    IdentityAssertionEvidenceClass::Candidate
                        | IdentityAssertionEvidenceClass::Disputed
                ))
        {
            return Err(IdentityAssertionError::InvalidStatus);
        }
        Ok(Self {
            assertion_id,
            workspace_id,
            record_id,
            source_external_identifier_id,
            target,
            relation,
            coverage,
            episode_links,
            evidence_class,
            evidence,
            id_source,
            source_version,
            authority,
            reasoning,
            initial_status,
            created_at: created_at.value(),
        })
    }

    pub const fn assertion_id(&self) -> IdentityAssertionId {
        self.assertion_id
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn source_external_identifier_id(&self) -> ExternalIdentifierId {
        self.source_external_identifier_id
    }

    pub const fn target(&self) -> &ExternalIdentifierClaim {
        &self.target
    }

    pub const fn relation(&self) -> IdentityAssertionRelation {
        self.relation
    }

    pub fn coverage(&self) -> &[IdentityCoverageSegment] {
        &self.coverage
    }

    pub fn episode_links(&self) -> &[IdentityEpisodeLink] {
        &self.episode_links
    }

    pub const fn evidence_class(&self) -> IdentityAssertionEvidenceClass {
        self.evidence_class
    }

    pub fn evidence(&self) -> &[IdentityAssertionEvidence] {
        &self.evidence
    }

    pub fn id_source(&self) -> &str {
        &self.id_source
    }

    pub fn source_version(&self) -> Option<&str> {
        self.source_version.as_deref()
    }

    pub fn authority(&self) -> Option<&str> {
        self.authority.as_deref()
    }

    pub fn reasoning(&self) -> Option<&str> {
        self.reasoning.as_deref()
    }

    pub const fn initial_status(&self) -> IdentityAssertionStatus {
        self.initial_status
    }

    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Derive the effective state from this assertion's complete lifecycle.
    ///
    /// The caller owns loading every event for this assertion in sequence order.
    /// A mismatched, incomplete, reordered, or invalid chain fails closed.
    pub fn effective_status(
        &self,
        lifecycle_events: &[IdentityAssertionLifecycleEvent],
    ) -> Result<IdentityAssertionStatus, IdentityAssertionLifecycleError> {
        let mut status = self.initial_status;
        let mut expected_sequence = 1_u32;
        let mut prior_time = self.created_at;
        for event in lifecycle_events {
            if event.assertion_id() != self.assertion_id {
                return Err(IdentityAssertionLifecycleError::AssertionMismatch);
            }
            if event.sequence() != expected_sequence {
                return Err(IdentityAssertionLifecycleError::InvalidSequence);
            }
            if event.previous_status() != status || !status.can_transition_to(event.status()) {
                return Err(IdentityAssertionLifecycleError::InvalidTransition);
            }
            if event.occurred_at() < prior_time {
                return Err(IdentityAssertionLifecycleError::InvalidTime);
            }
            status = event.status();
            prior_time = event.occurred_at();
            expected_sequence = expected_sequence
                .checked_add(1)
                .ok_or(IdentityAssertionLifecycleError::InvalidSequence)?;
        }
        Ok(status)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum IdentityAssertionLifecycleError {
    #[error("identity assertion lifecycle event references another assertion")]
    AssertionMismatch,
    #[error("identity assertion lifecycle sequence is incomplete or out of order")]
    InvalidSequence,
    #[error("identity assertion lifecycle transition is not permitted")]
    InvalidTransition,
    #[error("identity assertion lifecycle event predates its assertion or prior event")]
    InvalidTime,
    #[error("identity assertion rejection, dispute, or revocation requires evidence")]
    MissingEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityAssertionLifecycleEvent {
    assertion_id: IdentityAssertionId,
    sequence: u32,
    previous_status: IdentityAssertionStatus,
    status: IdentityAssertionStatus,
    reviewer_client_id: ClientId,
    occurred_at: DateTime<Utc>,
    evidence_digest: Option<Sha256Digest>,
}

impl IdentityAssertionLifecycleEvent {
    pub fn try_new(
        assertion_id: IdentityAssertionId,
        sequence: u32,
        previous_status: IdentityAssertionStatus,
        status: IdentityAssertionStatus,
        reviewer_client_id: ClientId,
        occurred_at: ReceivedAt,
        evidence_digest: Option<Sha256Digest>,
    ) -> Result<Self, IdentityAssertionLifecycleError> {
        if sequence == 0 {
            return Err(IdentityAssertionLifecycleError::InvalidSequence);
        }
        if !previous_status.can_transition_to(status) {
            return Err(IdentityAssertionLifecycleError::InvalidTransition);
        }
        if matches!(
            status,
            IdentityAssertionStatus::Disputed
                | IdentityAssertionStatus::Rejected
                | IdentityAssertionStatus::Revoked
        ) && evidence_digest.is_none()
        {
            return Err(IdentityAssertionLifecycleError::MissingEvidence);
        }
        Ok(Self {
            assertion_id,
            sequence,
            previous_status,
            status,
            reviewer_client_id,
            occurred_at: occurred_at.value(),
            evidence_digest,
        })
    }

    pub const fn assertion_id(&self) -> IdentityAssertionId {
        self.assertion_id
    }

    pub const fn sequence(&self) -> u32 {
        self.sequence
    }

    pub const fn previous_status(&self) -> IdentityAssertionStatus {
        self.previous_status
    }

    pub const fn status(&self) -> IdentityAssertionStatus {
        self.status
    }

    pub const fn reviewer_client_id(&self) -> ClientId {
        self.reviewer_client_id
    }

    pub const fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    pub const fn evidence_digest(&self) -> Option<&Sha256Digest> {
        self.evidence_digest.as_ref()
    }
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn valid_provenance_text(value: &str, max_bytes: usize) -> bool {
    value.chars().count() >= 3 && valid_text(value, max_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Grain;
    use chrono::TimeZone;

    fn at() -> ReceivedAt {
        ReceivedAt::from_application_clock(
            Utc.timestamp_opt(1_800_000_000, 0).single().expect("time"),
        )
    }

    fn claim(namespace: &str, grain: Grain, value: &str) -> ExternalIdentifierClaim {
        ExternalIdentifierClaim::try_new(namespace, grain, value).expect("identifier")
    }

    fn evidence(method: IdentityEvidenceMethod) -> IdentityAssertionEvidence {
        IdentityAssertionEvidence::try_new(
            method,
            "pinned source record",
            Some("pinned-source-root".to_owned()),
            (method == IdentityEvidenceMethod::HumanVerified).then(|| "gh:reviewer".to_owned()),
            NaiveDate::from_ymd_opt(2026, 8, 30).expect("date"),
            None,
        )
        .expect("evidence")
    }

    #[allow(clippy::too_many_arguments)]
    fn assertion_result(
        source: &ExternalIdentifierClaim,
        target: ExternalIdentifierClaim,
        relation: IdentityAssertionRelation,
        coverage: Vec<IdentityCoverageSegment>,
        episode_links: Vec<IdentityEpisodeLink>,
        evidence_class: IdentityAssertionEvidenceClass,
        evidence: Vec<IdentityAssertionEvidence>,
        id_source: &str,
        source_version: Option<String>,
        authority: Option<String>,
        reasoning: Option<String>,
        initial_status: IdentityAssertionStatus,
    ) -> Result<IdentityAssertion, IdentityAssertionError> {
        IdentityAssertion::try_new(
            IdentityAssertionId::new_v7(),
            WorkspaceId::new_v7(),
            RecordId::new_v7(),
            ExternalIdentifierId::new_v7(),
            source,
            target,
            relation,
            coverage,
            episode_links,
            evidence_class,
            evidence,
            id_source,
            source_version,
            authority,
            reasoning,
            initial_status,
            at(),
        )
    }

    #[test]
    fn exact_verified_assertion_retains_direction_and_provenance() {
        let source = claim("mal.anime", Grain::Release, "49894");
        let target = claim("imdb.title", Grain::Release, "tt28254942");
        let assertion = IdentityAssertion::try_new(
            IdentityAssertionId::new_v7(),
            WorkspaceId::new_v7(),
            RecordId::new_v7(),
            ExternalIdentifierId::new_v7(),
            &source,
            target.clone(),
            IdentityAssertionRelation::Exact,
            Vec::new(),
            Vec::new(),
            IdentityAssertionEvidenceClass::Verified,
            vec![evidence(IdentityEvidenceMethod::HumanVerified)],
            "anime-crosswalk-mappings:release",
            Some("dee4c1f4808d656b7ca71da584a8af95a2653277".to_owned()),
            None,
            None,
            IdentityAssertionStatus::Accepted,
            at(),
        )
        .expect("accepted assertion");

        assert_eq!(assertion.target(), &target);
        assert_eq!(assertion.relation(), IdentityAssertionRelation::Exact);
        assert_eq!(
            assertion.initial_status(),
            IdentityAssertionStatus::Accepted
        );
        assert_eq!(assertion.evidence().len(), 1);
    }

    #[test]
    fn unsafe_assertion_shapes_fail_closed() {
        let source = claim("mal.anime", Grain::Release, "49894");
        let target = claim("tmdb.tv", Grain::Series, "1399");
        let base = || {
            (
                IdentityAssertionId::new_v7(),
                WorkspaceId::new_v7(),
                RecordId::new_v7(),
                ExternalIdentifierId::new_v7(),
            )
        };

        assert!(IdentityAssertionEvidence::try_new(
            IdentityEvidenceMethod::UpstreamDeclared,
            "x",
            Some("root".to_owned()),
            None,
            NaiveDate::from_ymd_opt(2026, 8, 30).expect("date"),
            None,
        )
        .is_err());

        let (assertion_id, workspace_id, record_id, source_id) = base();
        assert!(IdentityAssertion::try_new(
            assertion_id,
            workspace_id,
            record_id,
            source_id,
            &source,
            target.clone(),
            IdentityAssertionRelation::SubsetOf,
            Vec::new(),
            Vec::new(),
            IdentityAssertionEvidenceClass::Verified,
            vec![evidence(IdentityEvidenceMethod::HumanVerified)],
            "source:route",
            None,
            None,
            None,
            IdentityAssertionStatus::Accepted,
            at(),
        )
        .is_err());

        let (assertion_id, workspace_id, record_id, source_id) = base();
        assert!(IdentityAssertion::try_new(
            assertion_id,
            workspace_id,
            record_id,
            source_id,
            &source,
            target,
            IdentityAssertionRelation::Exact,
            Vec::new(),
            Vec::new(),
            IdentityAssertionEvidenceClass::Candidate,
            vec![evidence(IdentityEvidenceMethod::HeuristicTitleMatch)],
            "source:route",
            None,
            None,
            None,
            IdentityAssertionStatus::Accepted,
            at(),
        )
        .is_err());

        assert!(IdentityCoverageSegment::try_new(
            IdentityCoverageMode::Flat,
            None,
            IdentityNumberingSpace::Regular,
            IdentityOrdering::Provider,
            u32::MAX,
            u32::MAX,
            1,
            None,
        )
        .is_err());

        let duplicate_root_evidence = ["source-a", "source-b"]
            .map(|observed_source| {
                IdentityAssertionEvidence::try_new(
                    IdentityEvidenceMethod::UpstreamDeclared,
                    observed_source,
                    Some("shared-upstream".to_owned()),
                    None,
                    NaiveDate::from_ymd_opt(2026, 8, 30).expect("date"),
                    None,
                )
                .expect("evidence")
            })
            .into_iter()
            .collect();
        let (assertion_id, workspace_id, record_id, source_id) = base();
        assert!(IdentityAssertion::try_new(
            assertion_id,
            workspace_id,
            record_id,
            source_id,
            &source,
            claim("imdb.title", Grain::Release, "tt28254942"),
            IdentityAssertionRelation::Exact,
            Vec::new(),
            Vec::new(),
            IdentityAssertionEvidenceClass::Corroborated,
            duplicate_root_evidence,
            "source:route",
            None,
            None,
            None,
            IdentityAssertionStatus::Accepted,
            at(),
        )
        .is_err());
    }

    #[test]
    fn assertion_admission_controls_fail_closed() {
        let source = claim("mal.anime", Grain::Release, "49894");
        let target = || claim("imdb.title", Grain::Release, "tt28254942");
        let verified = || vec![evidence(IdentityEvidenceMethod::HumanVerified)];
        let coverage = IdentityCoverageSegment::try_new(
            IdentityCoverageMode::Flat,
            None,
            IdentityNumberingSpace::Regular,
            IdentityOrdering::Provider,
            1,
            1,
            0,
            Some("*".to_owned()),
        )
        .expect("coverage");
        let episode_link =
            IdentityEpisodeLink::try_new(vec![1], vec![1], IdentityEpisodeLinkKind::Exact, None)
                .expect("episode link");

        assert!(assertion_result(
            &source,
            target(),
            IdentityAssertionRelation::Exact,
            vec![coverage; MAX_IDENTITY_COVERAGE_SEGMENTS + 1],
            Vec::new(),
            IdentityAssertionEvidenceClass::Verified,
            verified(),
            "source:route",
            None,
            None,
            None,
            IdentityAssertionStatus::Candidate,
        )
        .is_err());
        assert!(assertion_result(
            &source,
            target(),
            IdentityAssertionRelation::Exact,
            Vec::new(),
            vec![episode_link; MAX_IDENTITY_EPISODE_LINKS + 1],
            IdentityAssertionEvidenceClass::Verified,
            verified(),
            "source:route",
            None,
            None,
            None,
            IdentityAssertionStatus::Candidate,
        )
        .is_err());
        assert!(assertion_result(
            &source,
            target(),
            IdentityAssertionRelation::Exact,
            Vec::new(),
            Vec::new(),
            IdentityAssertionEvidenceClass::Verified,
            Vec::new(),
            "source:route",
            None,
            None,
            None,
            IdentityAssertionStatus::Candidate,
        )
        .is_err());
        assert!(assertion_result(
            &source,
            source.clone(),
            IdentityAssertionRelation::Exact,
            Vec::new(),
            Vec::new(),
            IdentityAssertionEvidenceClass::Verified,
            verified(),
            "source:route",
            None,
            None,
            None,
            IdentityAssertionStatus::Candidate,
        )
        .is_err());
        assert!(assertion_result(
            &source,
            target(),
            IdentityAssertionRelation::Exact,
            Vec::new(),
            Vec::new(),
            IdentityAssertionEvidenceClass::Verified,
            vec![
                evidence(IdentityEvidenceMethod::HumanVerified),
                evidence(IdentityEvidenceMethod::HumanVerified),
            ],
            "source:route",
            None,
            None,
            None,
            IdentityAssertionStatus::Candidate,
        )
        .is_err());

        for (id_source, source_version, authority) in [
            ("x", None, None),
            ("source:route", Some(String::new()), None),
            ("source:route", None, Some("\n".to_owned())),
        ] {
            assert!(assertion_result(
                &source,
                target(),
                IdentityAssertionRelation::Exact,
                Vec::new(),
                Vec::new(),
                IdentityAssertionEvidenceClass::Verified,
                verified(),
                id_source,
                source_version,
                authority,
                None,
                IdentityAssertionStatus::Candidate,
            )
            .is_err());
        }

        assert!(assertion_result(
            &source,
            target(),
            IdentityAssertionRelation::NotSameAs,
            Vec::new(),
            Vec::new(),
            IdentityAssertionEvidenceClass::Verified,
            verified(),
            "source:route",
            None,
            None,
            Some("too short".to_owned()),
            IdentityAssertionStatus::Candidate,
        )
        .is_err());
        assert!(assertion_result(
            &source,
            target(),
            IdentityAssertionRelation::Exact,
            Vec::new(),
            Vec::new(),
            IdentityAssertionEvidenceClass::Asserted,
            vec![evidence(IdentityEvidenceMethod::RightsholderAsserted)],
            "source:route",
            None,
            None,
            None,
            IdentityAssertionStatus::Candidate,
        )
        .is_err());
        assert!(assertion_result(
            &source,
            target(),
            IdentityAssertionRelation::Exact,
            Vec::new(),
            Vec::new(),
            IdentityAssertionEvidenceClass::Asserted,
            vec![evidence(IdentityEvidenceMethod::UpstreamDeclared)],
            "source:route",
            None,
            Some("rightsholder:studio".to_owned()),
            None,
            IdentityAssertionStatus::Candidate,
        )
        .is_err());
        assert!(assertion_result(
            &source,
            target(),
            IdentityAssertionRelation::Exact,
            Vec::new(),
            Vec::new(),
            IdentityAssertionEvidenceClass::Asserted,
            vec![evidence(IdentityEvidenceMethod::RightsholderAsserted)],
            "source:route",
            None,
            Some("rightsholder:studio".to_owned()),
            None,
            IdentityAssertionStatus::Candidate,
        )
        .is_ok());
        let unreviewed = IdentityAssertionEvidence::try_new(
            IdentityEvidenceMethod::HumanVerified,
            "pinned source record",
            Some("pinned-source-root".to_owned()),
            None,
            NaiveDate::from_ymd_opt(2026, 8, 30).expect("date"),
            None,
        )
        .expect("unreviewed evidence");
        assert!(assertion_result(
            &source,
            target(),
            IdentityAssertionRelation::Exact,
            Vec::new(),
            Vec::new(),
            IdentityAssertionEvidenceClass::Verified,
            vec![unreviewed],
            "source:route",
            None,
            None,
            None,
            IdentityAssertionStatus::Candidate,
        )
        .is_err());
    }

    #[test]
    fn lifecycle_is_append_only_and_negative_transitions_need_evidence() {
        let assertion_id = IdentityAssertionId::new_v7();
        assert!(IdentityAssertionLifecycleEvent::try_new(
            assertion_id,
            1,
            IdentityAssertionStatus::Candidate,
            IdentityAssertionStatus::Accepted,
            ClientId::new_v7(),
            at(),
            None,
        )
        .is_ok());
        assert!(IdentityAssertionLifecycleEvent::try_new(
            assertion_id,
            2,
            IdentityAssertionStatus::Accepted,
            IdentityAssertionStatus::Revoked,
            ClientId::new_v7(),
            at(),
            None,
        )
        .is_err());
    }

    #[test]
    fn lifecycle_transition_matrix_is_exact() {
        let statuses = [
            IdentityAssertionStatus::Candidate,
            IdentityAssertionStatus::Accepted,
            IdentityAssertionStatus::Disputed,
            IdentityAssertionStatus::Rejected,
            IdentityAssertionStatus::Revoked,
        ];
        let allowed = [
            (
                IdentityAssertionStatus::Candidate,
                IdentityAssertionStatus::Accepted,
            ),
            (
                IdentityAssertionStatus::Candidate,
                IdentityAssertionStatus::Disputed,
            ),
            (
                IdentityAssertionStatus::Candidate,
                IdentityAssertionStatus::Rejected,
            ),
            (
                IdentityAssertionStatus::Accepted,
                IdentityAssertionStatus::Disputed,
            ),
            (
                IdentityAssertionStatus::Accepted,
                IdentityAssertionStatus::Revoked,
            ),
            (
                IdentityAssertionStatus::Disputed,
                IdentityAssertionStatus::Accepted,
            ),
            (
                IdentityAssertionStatus::Disputed,
                IdentityAssertionStatus::Rejected,
            ),
            (
                IdentityAssertionStatus::Disputed,
                IdentityAssertionStatus::Revoked,
            ),
        ];

        for previous in statuses {
            for next in statuses {
                assert_eq!(
                    previous.can_transition_to(next),
                    allowed.contains(&(previous, next)),
                    "unexpected {previous:?} -> {next:?} transition",
                );
            }
        }
    }

    #[test]
    fn effective_status_validates_the_complete_lifecycle_chain() {
        let source = claim("mal.anime", Grain::Release, "49894");
        let assertion = IdentityAssertion::try_new(
            IdentityAssertionId::new_v7(),
            WorkspaceId::new_v7(),
            RecordId::new_v7(),
            ExternalIdentifierId::new_v7(),
            &source,
            claim("imdb.title", Grain::Release, "tt28254942"),
            IdentityAssertionRelation::Exact,
            Vec::new(),
            Vec::new(),
            IdentityAssertionEvidenceClass::Verified,
            vec![evidence(IdentityEvidenceMethod::HumanVerified)],
            "source:route",
            None,
            None,
            None,
            IdentityAssertionStatus::Candidate,
            at(),
        )
        .expect("candidate assertion");
        let accepted = IdentityAssertionLifecycleEvent::try_new(
            assertion.assertion_id(),
            1,
            IdentityAssertionStatus::Candidate,
            IdentityAssertionStatus::Accepted,
            ClientId::new_v7(),
            at(),
            None,
        )
        .expect("acceptance event");
        assert_eq!(
            assertion.effective_status(std::slice::from_ref(&accepted)),
            Ok(IdentityAssertionStatus::Accepted)
        );

        let wrong_assertion = IdentityAssertionLifecycleEvent::try_new(
            IdentityAssertionId::new_v7(),
            1,
            IdentityAssertionStatus::Candidate,
            IdentityAssertionStatus::Accepted,
            ClientId::new_v7(),
            at(),
            None,
        )
        .expect("foreign event");
        assert_eq!(
            assertion.effective_status(&[wrong_assertion]),
            Err(IdentityAssertionLifecycleError::AssertionMismatch)
        );

        let skipped = IdentityAssertionLifecycleEvent::try_new(
            assertion.assertion_id(),
            2,
            IdentityAssertionStatus::Candidate,
            IdentityAssertionStatus::Accepted,
            ClientId::new_v7(),
            at(),
            None,
        )
        .expect("individually valid skipped event");
        assert_eq!(
            assertion.effective_status(&[skipped]),
            Err(IdentityAssertionLifecycleError::InvalidSequence)
        );

        let wrong_previous = IdentityAssertionLifecycleEvent::try_new(
            assertion.assertion_id(),
            2,
            IdentityAssertionStatus::Candidate,
            IdentityAssertionStatus::Rejected,
            ClientId::new_v7(),
            at(),
            Some(Sha256Digest::from_bytes(&[1; 32])),
        )
        .expect("individually valid mismatched transition");
        assert_eq!(
            assertion.effective_status(&[accepted, wrong_previous]),
            Err(IdentityAssertionLifecycleError::InvalidTransition)
        );

        let predating = IdentityAssertionLifecycleEvent::try_new(
            assertion.assertion_id(),
            1,
            IdentityAssertionStatus::Candidate,
            IdentityAssertionStatus::Accepted,
            ClientId::new_v7(),
            ReceivedAt::from_application_clock(
                Utc.timestamp_opt(1_799_999_999, 0)
                    .single()
                    .expect("earlier time"),
            ),
            None,
        )
        .expect("individually valid predating event");
        assert_eq!(
            assertion.effective_status(&[predating]),
            Err(IdentityAssertionLifecycleError::InvalidTime)
        );
    }

    #[test]
    fn collection_bounds_fail_before_normalization() {
        assert!(IdentityEpisodeLink::try_new(
            vec![1; MAX_IDENTITY_EPISODES_PER_LINK_SIDE + 1],
            vec![1],
            IdentityEpisodeLinkKind::Merges,
            None,
        )
        .is_err());

        let source = claim("mal.anime", Grain::Release, "49894");
        assert!(IdentityAssertion::try_new(
            IdentityAssertionId::new_v7(),
            WorkspaceId::new_v7(),
            RecordId::new_v7(),
            ExternalIdentifierId::new_v7(),
            &source,
            claim("imdb.title", Grain::Release, "tt28254942"),
            IdentityAssertionRelation::Exact,
            Vec::new(),
            Vec::new(),
            IdentityAssertionEvidenceClass::Verified,
            vec![
                evidence(IdentityEvidenceMethod::HumanVerified);
                MAX_IDENTITY_ASSERTION_EVIDENCE + 1
            ],
            "source:route",
            None,
            None,
            None,
            IdentityAssertionStatus::Candidate,
            at(),
        )
        .is_err());
    }
}
