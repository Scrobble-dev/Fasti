//! Synthetic command checks only; no current-revision or authorization proof.

use chrono::{DateTime, TimeDelta, Utc};
use fasti_application::*;
use fasti_domain::{AccessConsentRevisionId, RequestCorrelationId};
use static_assertions::assert_not_impl_any;
use std::fmt;

assert_not_impl_any!(GrantAccessConsentCommand:
    fmt::Debug, Clone, Default, serde::Serialize, serde::de::DeserializeOwned,
    From<RequestAccessContext>, From<ApplicationAccessContext>, From<AuthenticatedBrowserSession>,
    From<SecretMaterial>, From<PersonalAccessTokenSecret>);
assert_not_impl_any!(RevokeAccessConsentCommand:
    fmt::Debug, Clone, Default, serde::Serialize, serde::de::DeserializeOwned,
    From<RequestAccessContext>, From<ApplicationAccessContext>, From<AuthenticatedBrowserSession>,
    From<SecretMaterial>, From<PersonalAccessTokenSecret>);

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

#[test]
fn grant_and_revoke_retain_exact_revision_and_borrowed_browser_evidence() {
    let received = DateTime::<Utc>::UNIX_EPOCH + TimeDelta::nanoseconds(123_456_789);
    let correlation = RequestCorrelationId::new_v7();
    let scopes = AccessScopeSet::try_new(&[
        ScopeKey::ProfileStateRead,
        ScopeKey::IdentityRead,
        ScopeKey::IdentityWrite,
    ])
    .unwrap();
    for revision in [
        AccessConsentRevisionId::new_v7(),
        AccessConsentRevisionId::new_v7(),
    ] {
        let grant = GrantAccessConsentCommand::try_new(
            request(correlation, received),
            revision,
            scopes.clone(),
        )
        .unwrap();
        let revoke = RevokeAccessConsentCommand::new(request(correlation, received), revision);
        assert_eq!(grant.expected_current_revision(), revision);
        assert_eq!(revoke.expected_current_revision(), revision);
        assert_eq!(grant.scopes(), &scopes);
        assert_eq!(
            grant.scopes().scopes(),
            &[
                ScopeKey::IdentityWrite,
                ScopeKey::IdentityRead,
                ScopeKey::ProfileStateRead
            ]
        );
        for evidence in [grant.request(), revoke.request()] {
            let browser = evidence.browser_request();
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
            assert!(std::ptr::eq(browser, evidence.browser_request()));
            assert!(std::ptr::eq(
                browser.session_secret(),
                evidence.browser_request().session_secret()
            ));
            assert!(std::ptr::eq(
                browser.csrf_secret(),
                evidence.browser_request().csrf_secret()
            ));
        }
    }
}

#[test]
fn empty_scope_evidence_is_valid_but_withdrawal_requires_the_revoke_command() {
    let received = DateTime::<Utc>::UNIX_EPOCH;
    let correlation = RequestCorrelationId::new_v7();
    let revision = AccessConsentRevisionId::new_v7();
    let empty = AccessScopeSet::try_new(&[]).unwrap();
    assert!(empty.scopes().is_empty());
    let result =
        GrantAccessConsentCommand::try_new(request(correlation, received), revision, empty);
    assert_eq!(result.err(), Some(GrantAccessConsentInputError));
    assert!(GrantAccessConsentInputError
        .to_string()
        .contains("revoke consent"));
    let revoke = RevokeAccessConsentCommand::new(request(correlation, received), revision);
    assert_eq!(revoke.expected_current_revision(), revision);
    assert_eq!(
        revoke.request().browser_request().correlation_id(),
        correlation
    );
}
