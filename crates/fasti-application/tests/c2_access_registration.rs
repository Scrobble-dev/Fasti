//! Synthetic request structure only: no session, grant, owner, or recent-auth
//! evidence is established by these tests.

use chrono::{DateTime, TimeDelta, Utc};
use fasti_application::*;
use fasti_domain::{
    AccessCredentialName, ApplicationClientPurpose, ClientAuthenticationType, RequestCorrelationId,
};
use static_assertions::assert_not_impl_any;
use std::{fmt, time::Duration};

assert_not_impl_any!(AccessAdministrationRequest:
    fmt::Debug, Clone, Default, serde::Serialize, serde::de::DeserializeOwned,
    From<RequestAccessContext>, From<ApplicationAccessContext>,
    From<AuthenticatedBrowserSession>, From<SecretMaterial>, From<PersonalAccessTokenSecret>);
assert_not_impl_any!(RegisterAccessClientCommand:
    fmt::Debug, Clone, Default, serde::Serialize, serde::de::DeserializeOwned,
    From<RequestAccessContext>, From<ApplicationAccessContext>,
    From<AuthenticatedBrowserSession>, From<SecretMaterial>, From<PersonalAccessTokenSecret>);

fn request(
    correlation: RequestCorrelationId,
    received: DateTime<Utc>,
) -> AccessAdministrationRequest {
    let boundary = BrowserRequestBoundaryPolicy::try_new("http://127.0.0.1:8420", "127.0.0.1:8420")
        .unwrap()
        .validate(Some("http://127.0.0.1:8420"), Some("127.0.0.1:8420"))
        .unwrap();
    AccessAdministrationRequest::new(BrowserSessionMutationCommand::new(
        correlation,
        SecretMaterial::from_bytes([1; 32]),
        SecretMaterial::from_bytes([2; 32]),
        boundary,
        received,
    ))
}

fn command(
    purpose: ApplicationClientPurpose,
    scopes: &[ScopeKey],
    received: DateTime<Utc>,
    expires: DateTime<Utc>,
    policy: TokenPolicy,
) -> Result<RegisterAccessClientCommand, RegisterAccessClientInputError> {
    RegisterAccessClientCommand::try_new(
        request(RequestCorrelationId::new_v7(), received),
        AccessCredentialName::try_new("Synthetic client").unwrap(),
        purpose,
        AccessScopeSet::try_new(scopes).unwrap(),
        expires,
        policy,
    )
}

#[test]
fn canonical_scope_order_and_independent_digest_vectors_include_empty_evidence() {
    let empty = AccessScopeSet::try_new(&[]).unwrap();
    assert!(empty.scopes().is_empty());
    // Fixed vectors computed independently with Node crypto SHA-256.
    assert_eq!(
        empty.digest().as_str(),
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    let canonical = [
        ScopeKey::IdentityWrite,
        ScopeKey::IdentityRead,
        ScopeKey::ProfileStateRead,
    ];
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let input = order.map(|index| canonical[index]);
        let set = AccessScopeSet::try_new(&input).unwrap();
        assert_eq!(set.scopes(), canonical);
        // This digest covers newline separators and the absence of a final newline.
        assert_eq!(
            set.digest().as_str(),
            "sha256:f8f439260ee1af6549c7acbd1b547a96c52fc25d286cc316d9013008161c89f8"
        );
    }
    let mut reversed = ScopeKey::ALL.to_vec();
    reversed.reverse();
    assert_eq!(
        AccessScopeSet::try_new(&reversed).unwrap().scopes(),
        ScopeKey::ALL
    );
    // Evidence is structurally valid even when scopes need later delegability checks.
    assert!(AccessScopeSet::try_new(&[ScopeKey::ClientEnroll]).is_ok());
}

#[test]
fn duplicate_and_oversized_scope_inputs_are_rejected() {
    for &scope in ScopeKey::ALL {
        assert_eq!(
            AccessScopeSet::try_new(&[scope, scope]),
            Err(AccessScopeSetInputError)
        );
        let mut duplicated = ScopeKey::ALL.to_vec();
        duplicated.push(scope);
        assert_eq!(
            AccessScopeSet::try_new(&duplicated),
            Err(AccessScopeSetInputError)
        );
    }
}

#[test]
fn registration_retains_browser_binding_and_explicit_expiry_without_owner_claims() {
    let received = DateTime::<Utc>::UNIX_EPOCH + TimeDelta::milliseconds(750);
    let expires = received + TimeDelta::days(2) - TimeDelta::milliseconds(250);
    for purpose in [
        ApplicationClientPurpose::Cli,
        ApplicationClientPurpose::Integration,
    ] {
        let correlation = RequestCorrelationId::new_v7();
        let name = AccessCredentialName::try_new("  Synthetic client  ").unwrap();
        let scopes =
            AccessScopeSet::try_new(&[ScopeKey::IdentityRead, ScopeKey::IdentityWrite]).unwrap();
        let registration = RegisterAccessClientCommand::try_new(
            request(correlation, received),
            name.clone(),
            purpose,
            scopes.clone(),
            expires,
            TokenPolicy::C2,
        )
        .unwrap_or_else(|_| panic!("in-range fractional expiry must remain valid"));
        let browser = registration.request().browser_request();
        assert_eq!(browser.correlation_id(), correlation);
        assert_eq!(browser.now(), received);
        assert!(browser
            .session_secret()
            .constant_time_eq(&SecretMaterial::from_bytes([1; 32])));
        assert!(browser
            .csrf_secret()
            .constant_time_eq(&SecretMaterial::from_bytes([2; 32])));
        assert!(!browser
            .session_secret()
            .constant_time_eq(browser.csrf_secret()));
        assert_eq!(registration.name(), &name);
        assert_eq!(registration.name().as_str(), "Synthetic client");
        assert_eq!(registration.scopes(), &scopes);
        assert_eq!(registration.expires_at(), expires);
        assert_eq!(registration.classification().purpose(), purpose);
        assert_eq!(
            registration.classification().authentication_type(),
            ClientAuthenticationType::Confidential
        );
    }
}

#[test]
fn registration_rejects_unsupported_purposes_empty_scopes_and_expiry_outside_bounds() {
    let received = DateTime::<Utc>::UNIX_EPOCH;
    let day = TimeDelta::days(1);
    let tick = TimeDelta::nanoseconds(1);
    for purpose in [
        ApplicationClientPurpose::Node,
        ApplicationClientPurpose::Device,
    ] {
        assert_eq!(
            command(
                purpose,
                &[ScopeKey::IdentityRead],
                received,
                received + day,
                TokenPolicy::C2
            )
            .err(),
            Some(RegisterAccessClientInputError::InvalidPurpose)
        );
    }
    assert_eq!(
        command(
            ApplicationClientPurpose::Cli,
            &[],
            received,
            received + day,
            TokenPolicy::C2
        )
        .err(),
        Some(RegisterAccessClientInputError::EmptyScopes)
    );
    for lifetime in [-tick, TimeDelta::zero(), day - tick, day * 365 + tick] {
        assert_eq!(
            command(
                ApplicationClientPurpose::Cli,
                &[ScopeKey::IdentityRead],
                received,
                received + lifetime,
                TokenPolicy::C2
            )
            .err(),
            Some(RegisterAccessClientInputError::InvalidExpiry)
        );
    }
    for lifetime in [day, day + tick, day * 365 - tick, day * 365] {
        let expires = received + lifetime;
        let accepted = command(
            ApplicationClientPurpose::Cli,
            &[ScopeKey::IdentityRead],
            received,
            expires,
            TokenPolicy::C2,
        )
        .unwrap_or_else(|_| panic!("inclusive lifetime boundary must be valid"));
        assert_eq!(accepted.expires_at(), expires);
    }
}

#[test]
fn explicit_policy_bounds_stay_whole_days_while_absolute_expiry_can_be_fractional() {
    let day = Duration::from_secs(86_400);
    assert!(TokenPolicy::try_new(
        day,
        day * 30,
        day * 365,
        day + Duration::from_nanos(1),
        day * 4
    )
    .is_err());
    let policy = TokenPolicy::try_new(day, day * 30, day * 365, day * 2, day * 4).unwrap();
    let received = DateTime::<Utc>::UNIX_EPOCH + TimeDelta::milliseconds(250);
    for (lifetime, accepted) in [
        (TimeDelta::days(2) - TimeDelta::nanoseconds(1), false),
        (TimeDelta::days(2), true),
        (TimeDelta::days(3) - TimeDelta::milliseconds(250), true),
        (TimeDelta::days(4), true),
        (TimeDelta::days(4) + TimeDelta::nanoseconds(1), false),
    ] {
        let result = command(
            ApplicationClientPurpose::Integration,
            &[ScopeKey::IdentityRead],
            received,
            received + lifetime,
            policy,
        );
        assert_eq!(result.is_ok(), accepted);
        if let Ok(registration) = result {
            assert_eq!(registration.expires_at(), received + lifetime);
            assert_eq!(registration.request().browser_request().now(), received);
        }
    }
}
