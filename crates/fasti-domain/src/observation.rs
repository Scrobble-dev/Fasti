use serde::Serialize;

use crate::{
    ClientId, EvidenceReference, ObservationId, ObservedAt, OccurredAt, ProfileId, ReceivedAt,
    WorkspaceId,
};

/// Resolution state produced when an observation enters the domain without an
/// identity decision.
///
/// Later bodies may add outcomes without adding Record or Occurrence mutation
/// to observation construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationResolution {
    Unresolved,
}

/// Immutable source observation accepted for later interpretation.
///
/// `Observation` intentionally contains neither a Record nor an Occurrence ID:
/// acceptance preserves source evidence without inventing an identity or
/// smuggling a resolution mutation into construction. It is serialize-only so
/// transport callers cannot deserialize and inject the server-owned
/// `ReceivedAt`; the application acceptance boundary is the sole intended
/// caller of `new_unresolved`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Observation {
    observation_id: ObservationId,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    source_client_id: ClientId,
    evidence: EvidenceReference,
    occurred_at: Option<OccurredAt>,
    observed_at: ObservedAt,
    received_at: ReceivedAt,
}

impl Observation {
    #[allow(clippy::too_many_arguments)]
    pub fn new_unresolved(
        observation_id: ObservationId,
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
        source_client_id: ClientId,
        evidence: EvidenceReference,
        occurred_at: Option<OccurredAt>,
        observed_at: ObservedAt,
        received_at: ReceivedAt,
    ) -> (Self, ObservationResolution) {
        (
            Self {
                observation_id,
                workspace_id,
                profile_id,
                source_client_id,
                evidence,
                occurred_at,
                observed_at,
                received_at,
            },
            ObservationResolution::Unresolved,
        )
    }

    pub fn observation_id(&self) -> ObservationId {
        self.observation_id
    }

    pub fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub fn source_client_id(&self) -> ClientId {
        self.source_client_id
    }

    pub fn evidence(&self) -> &EvidenceReference {
        &self.evidence
    }

    pub fn occurred_at(&self) -> Option<&OccurredAt> {
        self.occurred_at.as_ref()
    }

    pub fn observed_at(&self) -> &ObservedAt {
        &self.observed_at
    }

    pub fn received_at(&self) -> ReceivedAt {
        self.received_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClaimedTrust, EvidenceId, Sha256Digest};
    use chrono::{TimeZone, Utc};
    use static_assertions::assert_not_impl_any;

    assert_not_impl_any!(Observation: serde::de::DeserializeOwned);

    fn evidence() -> EvidenceReference {
        EvidenceReference::new(
            EvidenceId::new_v7(),
            Sha256Digest::parse(format!("sha256:{}", "12".repeat(32))).expect("canonical digest"),
            42,
        )
    }

    #[test]
    fn unresolved_construction_preserves_every_source_and_server_value() {
        let observation_id = ObservationId::new_v7();
        let workspace_id = WorkspaceId::new_v7();
        let profile_id = ProfileId::new_v7();
        let source_client_id = ClientId::new_v7();
        let evidence = evidence();
        let occurred_at = OccurredAt::parse("2026-08-21", ClaimedTrust::SourceClaim)
            .expect("valid occurrence claim");
        let observed_at = ObservedAt::parse(
            "2026-08-21T23:14:15.120+05:30",
            ClaimedTrust::DeviceObserved,
        )
        .expect("valid observation claim");
        let received_at = ReceivedAt::from_application_clock(
            Utc.with_ymd_and_hms(2026, 8, 21, 17, 44, 16)
                .single()
                .expect("valid instant"),
        );

        let (observation, resolution) = Observation::new_unresolved(
            observation_id,
            workspace_id,
            profile_id,
            source_client_id,
            evidence.clone(),
            Some(occurred_at.clone()),
            observed_at.clone(),
            received_at,
        );

        assert_eq!(resolution, ObservationResolution::Unresolved);
        assert_eq!(observation.observation_id(), observation_id);
        assert_eq!(observation.workspace_id(), workspace_id);
        assert_eq!(observation.profile_id(), profile_id);
        assert_eq!(observation.source_client_id(), source_client_id);
        assert_eq!(observation.evidence(), &evidence);
        assert_eq!(observation.occurred_at(), Some(&occurred_at));
        assert_eq!(observation.observed_at(), &observed_at);
        assert_eq!(observation.received_at(), received_at);
    }

    #[test]
    fn unresolved_observation_allows_an_absent_occurrence_claim() {
        let (observation, resolution) = Observation::new_unresolved(
            ObservationId::new_v7(),
            WorkspaceId::new_v7(),
            ProfileId::new_v7(),
            ClientId::new_v7(),
            evidence(),
            None,
            ObservedAt::parse("2026-08-21T17:44:15Z", ClaimedTrust::DeviceObserved)
                .expect("valid observation claim"),
            ReceivedAt::from_application_clock(
                Utc.with_ymd_and_hms(2026, 8, 21, 17, 44, 16)
                    .single()
                    .expect("valid instant"),
            ),
        );

        assert_eq!(resolution, ObservationResolution::Unresolved);
        assert_eq!(observation.occurred_at(), None);
    }

    #[test]
    fn observation_wire_output_contains_owned_values_but_has_no_resolution_target() {
        let (observation, _) = Observation::new_unresolved(
            ObservationId::new_v7(),
            WorkspaceId::new_v7(),
            ProfileId::new_v7(),
            ClientId::new_v7(),
            evidence(),
            None,
            ObservedAt::parse("2026-08-21T17:44:15Z", ClaimedTrust::DeviceObserved)
                .expect("valid observation claim"),
            ReceivedAt::from_application_clock(
                Utc.with_ymd_and_hms(2026, 8, 21, 17, 44, 16)
                    .single()
                    .expect("valid instant"),
            ),
        );
        let value = serde_json::to_value(observation).expect("serialize observation");
        let object = value.as_object().expect("observation object");

        assert!(object.contains_key("received_at"));
        assert!(!object.contains_key("record_id"));
        assert!(!object.contains_key("occurrence_id"));
        assert!(!object.contains_key("resolution"));
    }
}
