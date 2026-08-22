//! Pure application-layer authorization policy.
//!
//! Adapters load an [`AccessSnapshot`] and present request identity through a
//! [`RequestAccessContext`]. This module decides only whether the capability
//! may proceed; it deliberately does not know how either value was obtained.

use crate::{AuthorizationKind, CapabilityKey, ScopeKey};
use fasti_domain::{ClientId, CredentialId, ProfileGrantId, ProfileId, WorkspaceId};
use std::{collections::HashSet, error::Error, fmt};

/// Current credential state loaded from the source of truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialStatus {
    Active,
    Revoked,
}

/// Current profile grant state loaded from the source of truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantStatus {
    Active,
    Revoked,
}

/// Identity and credential generation presented by one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestAccessContext {
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    client_id: ClientId,
    credential_id: CredentialId,
    grant_id: ProfileGrantId,
    presented_credential_epoch: u64,
}

impl RequestAccessContext {
    pub const fn new(
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
        client_id: ClientId,
        credential_id: CredentialId,
        grant_id: ProfileGrantId,
        presented_credential_epoch: u64,
    ) -> Self {
        Self {
            workspace_id,
            profile_id,
            client_id,
            credential_id,
            grant_id,
            presented_credential_epoch,
        }
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub const fn client_id(&self) -> ClientId {
        self.client_id
    }

    pub const fn credential_id(&self) -> CredentialId {
        self.credential_id
    }

    pub const fn grant_id(&self) -> ProfileGrantId {
        self.grant_id
    }

    pub const fn presented_credential_epoch(&self) -> u64 {
        self.presented_credential_epoch
    }
}

/// Current authorization facts, loaded atomically from the source of truth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessSnapshot {
    state: AccessState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AccessState {
    BootstrapOpen,
    BootstrapClosed,
    Established(EstablishedAccess),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EstablishedAccess {
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    client_id: ClientId,
    credential_id: CredentialId,
    grant_id: ProfileGrantId,
    credential_status: CredentialStatus,
    grant_status: GrantStatus,
    current_credential_epoch: u64,
    granted_scopes: HashSet<ScopeKey>,
}

impl AccessSnapshot {
    /// An explicit fresh-node state. Absence of a snapshot never means this.
    pub const fn bootstrap_open() -> Self {
        Self {
            state: AccessState::BootstrapOpen,
        }
    }

    /// An explicitly locked bootstrap with no established access facts.
    pub const fn bootstrap_closed() -> Self {
        Self {
            state: AccessState::BootstrapClosed,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn established(
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
        client_id: ClientId,
        credential_id: CredentialId,
        grant_id: ProfileGrantId,
        credential_status: CredentialStatus,
        grant_status: GrantStatus,
        current_credential_epoch: u64,
        granted_scopes: impl IntoIterator<Item = ScopeKey>,
    ) -> Self {
        Self {
            state: AccessState::Established(EstablishedAccess {
                workspace_id,
                profile_id,
                client_id,
                credential_id,
                grant_id,
                credential_status,
                grant_status,
                current_credential_epoch,
                granted_scopes: granted_scopes.into_iter().collect(),
            }),
        }
    }

    pub const fn is_bootstrap_open(&self) -> bool {
        matches!(self.state, AccessState::BootstrapOpen)
    }

    pub const fn is_established(&self) -> bool {
        matches!(self.state, AccessState::Established(_))
    }

    pub const fn workspace_id(&self) -> Option<WorkspaceId> {
        match &self.state {
            AccessState::Established(access) => Some(access.workspace_id),
            AccessState::BootstrapOpen | AccessState::BootstrapClosed => None,
        }
    }

    pub const fn profile_id(&self) -> Option<ProfileId> {
        match &self.state {
            AccessState::Established(access) => Some(access.profile_id),
            AccessState::BootstrapOpen | AccessState::BootstrapClosed => None,
        }
    }

    pub const fn client_id(&self) -> Option<ClientId> {
        match &self.state {
            AccessState::Established(access) => Some(access.client_id),
            AccessState::BootstrapOpen | AccessState::BootstrapClosed => None,
        }
    }

    pub const fn credential_id(&self) -> Option<CredentialId> {
        match &self.state {
            AccessState::Established(access) => Some(access.credential_id),
            AccessState::BootstrapOpen | AccessState::BootstrapClosed => None,
        }
    }

    pub const fn grant_id(&self) -> Option<ProfileGrantId> {
        match &self.state {
            AccessState::Established(access) => Some(access.grant_id),
            AccessState::BootstrapOpen | AccessState::BootstrapClosed => None,
        }
    }

    pub const fn credential_status(&self) -> Option<CredentialStatus> {
        match &self.state {
            AccessState::Established(access) => Some(access.credential_status),
            AccessState::BootstrapOpen | AccessState::BootstrapClosed => None,
        }
    }

    pub const fn grant_status(&self) -> Option<GrantStatus> {
        match &self.state {
            AccessState::Established(access) => Some(access.grant_status),
            AccessState::BootstrapOpen | AccessState::BootstrapClosed => None,
        }
    }

    pub const fn current_credential_epoch(&self) -> Option<u64> {
        match &self.state {
            AccessState::Established(access) => Some(access.current_credential_epoch),
            AccessState::BootstrapOpen | AccessState::BootstrapClosed => None,
        }
    }

    pub fn has_scope(&self, scope: ScopeKey) -> bool {
        match &self.state {
            AccessState::Established(access) => access.granted_scopes.contains(&scope),
            AccessState::BootstrapOpen | AccessState::BootstrapClosed => false,
        }
    }

    fn established_access(&self) -> Option<&EstablishedAccess> {
        match &self.state {
            AccessState::Established(access) => Some(access),
            AccessState::BootstrapOpen | AccessState::BootstrapClosed => None,
        }
    }
}

/// The policy derived from one application capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizationRequirement {
    capability: CapabilityKey,
    kind: AuthorizationKind,
}

impl AuthorizationRequirement {
    /// Derive policy from the capability table; callers cannot weaken it.
    pub const fn for_capability(capability: CapabilityKey) -> Self {
        Self {
            capability,
            kind: capability.authorization_kind(),
        }
    }

    pub const fn capability(&self) -> CapabilityKey {
        self.capability
    }

    pub const fn is_unauthenticated(&self) -> bool {
        matches!(self.kind, AuthorizationKind::Unauthenticated)
    }

    pub const fn is_bootstrap_only(&self) -> bool {
        matches!(self.kind, AuthorizationKind::BootstrapOnly)
    }

    pub const fn required_scopes(&self) -> &'static [ScopeKey] {
        match self.kind {
            AuthorizationKind::Scoped => self.capability.required_scopes(),
            AuthorizationKind::Unauthenticated | AuthorizationKind::BootstrapOnly => &[],
        }
    }
}

/// Proof that the pure policy evaluator allowed one capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizedCapability {
    capability: CapabilityKey,
}

impl AuthorizedCapability {
    pub const fn capability(&self) -> CapabilityKey {
        self.capability
    }
}

/// Deliberately non-enumerating denial. Predicate failures are not exposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizationDenied;

impl fmt::Display for AuthorizationDenied {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("request is not authorized")
    }
}

impl Error for AuthorizationDenied {}

/// Evaluate current request and stored facts without adapter or runtime state.
pub fn authorize(
    requirement: &AuthorizationRequirement,
    request: Option<&RequestAccessContext>,
    snapshot: Option<&AccessSnapshot>,
) -> Result<AuthorizedCapability, AuthorizationDenied> {
    let allowed = match requirement.kind {
        AuthorizationKind::Unauthenticated => true,
        AuthorizationKind::BootstrapOnly => snapshot.is_some_and(AccessSnapshot::is_bootstrap_open),
        AuthorizationKind::Scoped => {
            let required_scopes = requirement.capability.required_scopes();
            match (
                request,
                snapshot.and_then(AccessSnapshot::established_access),
            ) {
                (Some(request), Some(access)) => {
                    request.workspace_id == access.workspace_id
                        && request.profile_id == access.profile_id
                        && request.client_id == access.client_id
                        && request.credential_id == access.credential_id
                        && request.grant_id == access.grant_id
                        && access.credential_status == CredentialStatus::Active
                        && access.grant_status == GrantStatus::Active
                        && request.presented_credential_epoch == access.current_credential_epoch
                        && required_scopes
                            .iter()
                            .all(|scope| access.granted_scopes.contains(scope))
                }
                _ => false,
            }
        }
    };

    allowed
        .then_some(AuthorizedCapability {
            capability: requirement.capability,
        })
        .ok_or(AuthorizationDenied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct Identities {
        workspace: WorkspaceId,
        other_workspace: WorkspaceId,
        profile: ProfileId,
        other_profile: ProfileId,
        client: ClientId,
        other_client: ClientId,
        credential: CredentialId,
        other_credential: CredentialId,
        grant: ProfileGrantId,
        other_grant: ProfileGrantId,
    }

    impl Identities {
        fn distinct() -> Self {
            Self {
                workspace: WorkspaceId::new_v7(),
                other_workspace: WorkspaceId::new_v7(),
                profile: ProfileId::new_v7(),
                other_profile: ProfileId::new_v7(),
                client: ClientId::new_v7(),
                other_client: ClientId::new_v7(),
                credential: CredentialId::new_v7(),
                other_credential: CredentialId::new_v7(),
                grant: ProfileGrantId::new_v7(),
                other_grant: ProfileGrantId::new_v7(),
            }
        }
    }

    fn request(ids: Identities, matches: [bool; 5], epoch: u64) -> RequestAccessContext {
        RequestAccessContext::new(
            if matches[0] {
                ids.workspace
            } else {
                ids.other_workspace
            },
            if matches[1] {
                ids.profile
            } else {
                ids.other_profile
            },
            if matches[2] {
                ids.client
            } else {
                ids.other_client
            },
            if matches[3] {
                ids.credential
            } else {
                ids.other_credential
            },
            if matches[4] {
                ids.grant
            } else {
                ids.other_grant
            },
            epoch,
        )
    }

    #[test]
    fn requirement_is_derived_from_capability_without_a_policy_override() {
        let health = AuthorizationRequirement::for_capability(CapabilityKey::SystemHealth);
        assert!(health.is_unauthenticated());
        assert!(health.required_scopes().is_empty());

        let initialize = AuthorizationRequirement::for_capability(CapabilityKey::InitializeNode);
        assert!(initialize.is_bootstrap_only());
        assert!(initialize.required_scopes().is_empty());

        for capability in CapabilityKey::ALL.iter().copied().filter(|capability| {
            !matches!(
                capability,
                CapabilityKey::SystemHealth | CapabilityKey::InitializeNode
            )
        }) {
            let requirement = AuthorizationRequirement::for_capability(capability);
            assert!(!requirement.is_unauthenticated());
            assert!(!requirement.is_bootstrap_only());
            assert_eq!(requirement.required_scopes(), capability.required_scopes());
        }
    }

    #[test]
    fn health_is_unauthenticated_and_initialization_is_explicitly_bootstrap_only() {
        let health = AuthorizationRequirement::for_capability(CapabilityKey::SystemHealth);
        assert_eq!(
            authorize(&health, None, None).unwrap().capability(),
            CapabilityKey::SystemHealth
        );

        let initialize = AuthorizationRequirement::for_capability(CapabilityKey::InitializeNode);
        assert!(authorize(&initialize, None, Some(&AccessSnapshot::bootstrap_open())).is_ok());
        assert!(authorize(&initialize, None, None).is_err());
        assert!(authorize(&initialize, None, Some(&AccessSnapshot::bootstrap_closed())).is_err());

        let ids = Identities::distinct();
        let established = AccessSnapshot::established(
            ids.workspace,
            ids.profile,
            ids.client,
            ids.credential,
            ids.grant,
            CredentialStatus::Active,
            GrantStatus::Active,
            1,
            [],
        );
        assert!(authorize(&initialize, None, Some(&established)).is_err());
    }

    #[test]
    fn scoped_allowance_is_equivalent_to_all_nine_predicates() {
        let ids = Identities::distinct();
        let requirement =
            AuthorizationRequirement::for_capability(CapabilityKey::DiscoverCapabilities);

        // Five identity matches, active credential, active grant, equal epoch,
        // and required scope: exhaust every truth-table row.
        for mask in 0_u16..(1_u16 << 9) {
            let predicate = |bit| mask & (1_u16 << bit) != 0_u16;
            let request = request(
                ids,
                [
                    predicate(0),
                    predicate(1),
                    predicate(2),
                    predicate(3),
                    predicate(4),
                ],
                7,
            );
            let snapshot = AccessSnapshot::established(
                ids.workspace,
                ids.profile,
                ids.client,
                ids.credential,
                ids.grant,
                if predicate(5) {
                    CredentialStatus::Active
                } else {
                    CredentialStatus::Revoked
                },
                if predicate(6) {
                    GrantStatus::Active
                } else {
                    GrantStatus::Revoked
                },
                if predicate(7) { 7 } else { 8 },
                predicate(8).then_some(ScopeKey::CapabilityRead),
            );

            assert_eq!(
                authorize(&requirement, Some(&request), Some(&snapshot)).is_ok(),
                mask == (1_u16 << 9) - 1,
                "unexpected authorization result for predicate mask {mask:09b}"
            );
        }
    }

    #[test]
    fn revocation_and_epoch_advancement_never_allow() {
        let ids = Identities::distinct();
        let requirement =
            AuthorizationRequirement::for_capability(CapabilityKey::DiscoverCapabilities);

        for presented_epoch in 0_u64..64 {
            let request = request(ids, [true; 5], presented_epoch);

            for (credential_status, grant_status) in [
                (CredentialStatus::Revoked, GrantStatus::Active),
                (CredentialStatus::Active, GrantStatus::Revoked),
                (CredentialStatus::Revoked, GrantStatus::Revoked),
            ] {
                let snapshot = AccessSnapshot::established(
                    ids.workspace,
                    ids.profile,
                    ids.client,
                    ids.credential,
                    ids.grant,
                    credential_status,
                    grant_status,
                    presented_epoch,
                    [ScopeKey::CapabilityRead],
                );
                assert!(authorize(&requirement, Some(&request), Some(&snapshot)).is_err());
            }

            let advanced = AccessSnapshot::established(
                ids.workspace,
                ids.profile,
                ids.client,
                ids.credential,
                ids.grant,
                CredentialStatus::Active,
                GrantStatus::Active,
                presented_epoch + 1,
                [ScopeKey::CapabilityRead],
            );
            assert!(authorize(&requirement, Some(&request), Some(&advanced)).is_err());
        }
    }

    #[test]
    fn denials_do_not_disclose_the_failed_predicate() {
        let ids = Identities::distinct();
        let requirement =
            AuthorizationRequirement::for_capability(CapabilityKey::DiscoverCapabilities);
        let request = request(ids, [false, true, true, true, true], 3);
        let snapshot = AccessSnapshot::established(
            ids.workspace,
            ids.profile,
            ids.client,
            ids.credential,
            ids.grant,
            CredentialStatus::Active,
            GrantStatus::Active,
            3,
            [ScopeKey::CapabilityRead],
        );

        let error = authorize(&requirement, Some(&request), Some(&snapshot)).unwrap_err();
        assert_eq!(error.to_string(), "request is not authorized");
        assert_eq!(format!("{error:?}"), "AuthorizationDenied");
    }
}
