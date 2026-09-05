//! Candidate behavior only. No listener, Fasti account, session or persistence.
use chrono::{DateTime, TimeZone, Utc};
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreIdToken, CoreIdTokenClaims, CoreIdTokenVerifier,
    CoreJsonWebKeySet, CoreJwsSigningAlgorithm, CoreProviderMetadata, CoreRsaPrivateSigningKey,
    CoreUserInfoClaims,
};
use openidconnect::{
    AccessToken, AccessTokenHash, Audience, AuthorizationCode, ClientId, CsrfToken,
    EmptyAdditionalClaims, EndSessionUrl, HttpRequest, IssuerUrl, JsonWebKeyId, LogoutRequest,
    Nonce, OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, PrivateSigningKey,
    RedirectUrl, RequestTokenError, StandardClaims, SubjectIdentifier, TokenResponse,
};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    future::Future,
    io,
    task::{Context, Poll, Waker},
};

const ISSUER: &str = "https://identity.example.test";
const CLIENT: &str = "qualification-only";
const NOW: i64 = 1_800_000_000;

fn instant(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0).single().unwrap()
}

fn issuer() -> IssuerUrl {
    IssuerUrl::new(ISSUER.into()).unwrap()
}

fn key() -> CoreRsaPrivateSigningKey {
    // Generated solely for this offline harness; never valid for a real service.
    CoreRsaPrivateSigningKey::from_pem(
        include_str!("synthetic-test-key.pem"),
        Some(JsonWebKeyId::new("test-key".into())),
    )
    .unwrap()
}

fn claims() -> CoreIdTokenClaims {
    CoreIdTokenClaims::new(
        issuer(),
        vec![Audience::new(CLIENT.into())],
        instant(NOW + 60),
        instant(NOW),
        StandardClaims::new(SubjectIdentifier::new("synthetic-person".into())),
        EmptyAdditionalClaims {},
    )
    .set_nonce(Some(Nonce::new("synthetic-nonce".into())))
}

fn sign(claims: CoreIdTokenClaims) -> CoreIdToken {
    CoreIdToken::new(
        claims,
        &key(),
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        Some(&AccessToken::new("synthetic-access-token".into())),
        None,
    )
    .unwrap()
}

fn verifier() -> CoreIdTokenVerifier<'static> {
    CoreIdTokenVerifier::new_public_client(
        ClientId::new(CLIENT.into()),
        issuer(),
        CoreJsonWebKeySet::new(vec![key().as_verification_key()]),
    )
    .set_time_fn(|| instant(NOW))
}

fn metadata() -> Value {
    json!({
        "issuer": ISSUER,
        "authorization_endpoint": format!("{ISSUER}/authorize"),
        "token_endpoint": format!("{ISSUER}/token"),
        "userinfo_endpoint": format!("{ISSUER}/userinfo"),
        "jwks_uri": format!("{ISSUER}/keys"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"]
    })
}

// Every supplied HTTP future is Ready. Pending is a test error, not a busy-loop executor.
fn ready<T>(future: impl Future<Output = T>) -> T {
    match std::pin::pin!(future)
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("in-memory qualification unexpectedly suspended"),
    }
}

#[test]
fn async_discovery_checks_issuer_before_fetching_keys() {
    for substituted in [false, true] {
        let calls = RefCell::new(Vec::new());
        let http = |request: HttpRequest| {
            assert_eq!(request.method(), "GET");
            assert!(!request.headers().contains_key("authorization"));
            calls.borrow_mut().push(request.uri().to_string());
            let mut document = metadata();
            if substituted {
                document["issuer"] = json!("https://other.example.test");
            }
            let body = if request.uri().path().ends_with("openid-configuration") {
                serde_json::to_vec(&document).unwrap()
            } else {
                serde_json::to_vec(&CoreJsonWebKeySet::new(vec![key().as_verification_key()]))
                    .unwrap()
            };
            std::future::ready(Ok::<_, io::Error>(
                openidconnect::http::Response::builder()
                    .header("content-type", "application/json")
                    .body(body)
                    .unwrap(),
            ))
        };
        let result = ready(CoreProviderMetadata::discover_async(issuer(), &http));
        assert_eq!(result.is_ok(), !substituted);
        let mut expected = vec![format!("{ISSUER}/.well-known/openid-configuration")];
        if !substituted {
            expected.push(format!("{ISSUER}/keys"));
        }
        assert_eq!(*calls.borrow(), expected);
    }
}

#[test]
fn malformed_discovery_never_fetches_keys() {
    for body in [b"{".to_vec(), b"{}".to_vec()] {
        let count = RefCell::new(0);
        let http = |_: HttpRequest| {
            *count.borrow_mut() += 1;
            std::future::ready(Ok::<_, io::Error>(
                openidconnect::http::Response::builder()
                    .header("content-type", "application/json")
                    .body(body.clone())
                    .unwrap(),
            ))
        };
        assert!(ready(CoreProviderMetadata::discover_async(issuer(), &http)).is_err());
        assert_eq!(*count.borrow(), 1);
    }
}

#[test]
fn authorization_request_uses_rfc7636_s256_and_returns_state_nonce() {
    let provider: CoreProviderMetadata = serde_json::from_value(metadata()).unwrap();
    let client = CoreClient::from_provider_metadata(provider, ClientId::new(CLIENT.into()), None)
        .set_redirect_uri(RedirectUrl::new("https://fasti.example.test/callback".into()).unwrap());
    // Public RFC 7636 section 4.2 vector, not a credential.
    let verifier = PkceCodeVerifier::new("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into());
    let (url, state, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            || CsrfToken::new("synthetic-state".into()),
            || Nonce::new("synthetic-nonce".into()),
        )
        .set_pkce_challenge(PkceCodeChallenge::from_code_verifier_sha256(&verifier))
        .url();
    assert_eq!(url.origin().ascii_serialization(), ISSUER);
    assert_eq!(url.path(), "/authorize");
    let query: std::collections::BTreeMap<_, _> = url.query_pairs().collect();
    assert_eq!(query.get("client_id").unwrap(), CLIENT);
    assert_eq!(
        query.get("redirect_uri").unwrap(),
        "https://fasti.example.test/callback"
    );
    assert_eq!(query.get("response_type").unwrap(), "code");
    assert_eq!(query.get("code_challenge_method").unwrap(), "S256");
    assert_eq!(
        query.get("code_challenge").unwrap(),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    assert_eq!(query.get("state").unwrap().as_ref(), state.secret());
    assert_eq!(query.get("nonce").unwrap().as_ref(), nonce.secret());
    assert!(!query.contains_key("code_verifier"));
    assert!(!query.contains_key("client_secret"));
}

#[test]
fn signed_claims_reject_independent_identity_and_time_mutations() {
    let nonce = Nonce::new("synthetic-nonce".into());
    assert!(sign(claims()).claims(&verifier(), &nonce).is_ok());
    let cases = [
        claims().set_issuer(IssuerUrl::new("https://other.example.test".into()).unwrap()),
        claims().set_audiences(vec![Audience::new("other-client".into())]),
        claims().set_audiences(vec![
            Audience::new(CLIENT.into()),
            Audience::new("other-client".into()),
        ]),
        claims().set_expiration(instant(NOW)),
        claims().set_nonce(None),
        claims().set_nonce(Some(Nonce::new("other-nonce".into()))),
    ];
    for (index, claim) in cases.into_iter().enumerate() {
        assert!(
            sign(claim).claims(&verifier(), &nonce).is_err(),
            "mutation {index}"
        );
    }
}

#[test]
fn signature_damage_and_key_removal_reject_previously_valid_token() {
    let token = sign(claims());
    let nonce = Nonce::new("synthetic-nonce".into());
    assert!(token.claims(&verifier(), &nonce).is_ok());
    let empty = CoreIdTokenVerifier::new_public_client(
        ClientId::new(CLIENT.into()),
        issuer(),
        CoreJsonWebKeySet::new(vec![]),
    )
    .set_time_fn(|| instant(NOW));
    assert!(token.claims(&empty, &nonce).is_err());
    let mut wire = token.to_string().into_bytes();
    let signature_start = wire.iter().rposition(|byte| *byte == b'.').unwrap() + 1;
    wire[signature_start] = if wire[signature_start] == b'A' {
        b'B'
    } else {
        b'A'
    };
    let damaged: CoreIdToken = String::from_utf8(wire).unwrap().parse().unwrap();
    assert!(damaged.claims(&verifier(), &nonce).is_err());
}

#[test]
fn explicit_key_set_replacement_accepts_overlap_then_rejects_retired_key() {
    let next_key = CoreRsaPrivateSigningKey::from_pem(
        include_str!("synthetic-rotation-key.pem"),
        Some(JsonWebKeyId::new("next-test-key".into())),
    )
    .unwrap();
    let old_token = sign(claims());
    let new_token = CoreIdToken::new(
        claims(),
        &next_key,
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        None,
        None,
    )
    .unwrap();
    let nonce = Nonce::new("synthetic-nonce".into());
    let old_public = key().as_verification_key();
    let new_public = next_key.as_verification_key();
    for (keys, expected) in [
        (vec![old_public.clone()], [true, false]),
        (vec![old_public, new_public.clone()], [true, true]),
        (vec![new_public], [false, true]),
    ] {
        let verifier = CoreIdTokenVerifier::new_public_client(
            ClientId::new(CLIENT.into()),
            issuer(),
            CoreJsonWebKeySet::new(keys),
        )
        .set_time_fn(|| instant(NOW));
        assert_eq!(
            [
                old_token.claims(&verifier, &nonce).is_ok(),
                new_token.claims(&verifier, &nonce).is_ok()
            ],
            expected
        );
    }
    // This tests explicit key-set replacement, not network refresh or retirement scheduling.
}

#[test]
fn issued_at_requires_explicit_hook_not_library_default() {
    let nonce = Nonce::new("synthetic-nonce".into());
    let bounded = verifier().set_issue_time_verifier_fn(|issued| {
        // Test policy only. Final Fasti skew/age policy remains an integration decision.
        if (NOW - 300..=NOW + 30).contains(&issued.timestamp()) {
            Ok(())
        } else {
            Err("issued-at outside qualification window".into())
        }
    });
    for (offset, accepted) in [
        (-301, false),
        (-300, true),
        (0, true),
        (30, true),
        (31, false),
    ] {
        let token = sign(claims().set_issue_time(instant(NOW + offset)));
        assert!(token.claims(&verifier(), &nonce).is_ok());
        assert_eq!(token.claims(&bounded, &nonce).is_ok(), accepted);
    }
}

#[test]
fn wrong_authorized_party_is_exposed_but_not_rejected_by_library() {
    let token = sign(claims().set_authorized_party(Some(ClientId::new("other-client".into()))));
    let nonce = Nonce::new("synthetic-nonce".into());
    let verifier = verifier();
    let verified = token.claims(&verifier, &nonce).unwrap();
    assert_ne!(verified.authorized_party().unwrap().as_str(), CLIENT);
    // This evidence requires a caller policy; it is not an acceptable sign-in result.
}

#[test]
fn access_token_hash_requires_caller_comparison() {
    let token = sign(claims());
    let nonce = Nonce::new("synthetic-nonce".into());
    let verifier = verifier();
    let verified = token.claims(&verifier, &nonce).unwrap();
    for (value, matches) in [
        ("synthetic-access-token", true),
        ("substituted-token", false),
    ] {
        let calculated = AccessTokenHash::from_token(
            &AccessToken::new(value.into()),
            token.signing_alg().unwrap(),
            token.signing_key(&verifier).unwrap(),
        )
        .unwrap();
        assert_eq!(
            verified.access_token_hash().unwrap() == &calculated,
            matches
        );
    }
}

#[test]
fn logout_builder_can_omit_id_token_hint() {
    let url = LogoutRequest::from(EndSessionUrl::new(format!("{ISSUER}/logout")).unwrap())
        .set_client_id(ClientId::new(CLIENT.into()))
        .http_get_url();
    let query: Vec<_> = url.query_pairs().collect();
    assert_eq!(query, [("client_id".into(), CLIENT.into())]);
    // URL construction does not prove any provider accepts hintless logout.
}

#[test]
fn code_exchange_posts_bound_form_and_requires_caller_to_demand_id_token() {
    let client = CoreClient::from_provider_metadata(
        serde_json::from_value::<CoreProviderMetadata>(metadata()).unwrap(),
        ClientId::new(CLIENT.into()),
        None,
    )
    .set_redirect_uri(RedirectUrl::new("https://fasti.example.test/callback".into()).unwrap());
    for with_id in [true, false] {
        let count = RefCell::new(0);
        let http = |request: HttpRequest| {
            *count.borrow_mut() += 1;
            assert_eq!(request.method(), "POST");
            assert_eq!(request.uri().to_string(), format!("{ISSUER}/token"));
            assert!(!request.headers().contains_key("authorization"));
            assert_eq!(
                request.headers()["content-type"],
                "application/x-www-form-urlencoded"
            );
            let pairs: Vec<_> =
                openidconnect::url::form_urlencoded::parse(request.body()).collect();
            assert_eq!(pairs.len(), 5);
            let form: std::collections::BTreeMap<_, _> = pairs.into_iter().collect();
            assert_eq!(form.len(), 5);
            assert_eq!(form.get("grant_type").unwrap(), "authorization_code");
            assert_eq!(form.get("code").unwrap(), "synthetic-code");
            assert_eq!(
                form.get("code_verifier").unwrap(),
                "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
            );
            assert_eq!(form.get("client_id").unwrap(), CLIENT);
            assert_eq!(
                form.get("redirect_uri").unwrap(),
                "https://fasti.example.test/callback"
            );
            let mut response =
                json!({"access_token":"synthetic-access-token", "token_type":"Bearer"});
            if with_id {
                response["id_token"] = serde_json::to_value(sign(claims())).unwrap();
            }
            std::future::ready(Ok::<_, io::Error>(
                openidconnect::http::Response::builder()
                    .header("content-type", "application/json")
                    .body(serde_json::to_vec(&response).unwrap())
                    .unwrap(),
            ))
        };
        let result = ready(
            client
                .exchange_code(AuthorizationCode::new("synthetic-code".into()))
                .unwrap()
                .set_pkce_verifier(PkceCodeVerifier::new(
                    "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into(),
                ))
                .request_async(&http),
        )
        .unwrap();
        assert_eq!(*count.borrow(), 1);
        assert_eq!(result.access_token().secret(), "synthetic-access-token");
        assert_eq!(result.id_token().is_some(), with_id);
        if let Some(token) = result.id_token() {
            assert!(token
                .claims(&verifier(), &Nonce::new("synthetic-nonce".into()))
                .is_ok());
        }
        // Parsing an OAuth response without an ID token succeeds; Fasti must reject sign-in.
    }
}

#[test]
fn token_transport_failure_is_not_retried_by_library() {
    let client = CoreClient::from_provider_metadata(
        serde_json::from_value::<CoreProviderMetadata>(metadata()).unwrap(),
        ClientId::new(CLIENT.into()),
        None,
    );
    let count = RefCell::new(0);
    let http = |_: HttpRequest| {
        *count.borrow_mut() += 1;
        std::future::ready(Err::<openidconnect::HttpResponse, _>(io::Error::new(
            io::ErrorKind::ConnectionReset,
            "synthetic lost response",
        )))
    };
    let result = ready(
        client
            .exchange_code(AuthorizationCode::new("synthetic-code".into()))
            .unwrap()
            .request_async(&http),
    );
    assert!(matches!(result, Err(RequestTokenError::Request(_))));
    assert_eq!(*count.borrow(), 1);
    // No claim about a server consuming the code or Fasti's durable recovery follows.
}

#[test]
fn token_parse_error_retains_response_bytes_that_must_not_be_logged() {
    let client = CoreClient::from_provider_metadata(
        serde_json::from_value::<CoreProviderMetadata>(metadata()).unwrap(),
        ClientId::new(CLIENT.into()),
        None,
    );
    let marker = b"synthetic-provider-echo-not-json";
    let http = |_: HttpRequest| {
        std::future::ready(Ok::<_, io::Error>(
            openidconnect::http::Response::builder()
                .header("content-type", "application/json")
                .body(marker.to_vec())
                .unwrap(),
        ))
    };
    let result = ready(
        client
            .exchange_code(AuthorizationCode::new("synthetic-code".into()))
            .unwrap()
            .request_async(&http),
    );
    match result {
        Err(RequestTokenError::Parse(_, body)) => assert_eq!(body, marker),
        _ => panic!("expected retained parse-error body"),
    }
}

#[test]
fn userinfo_requires_verified_subject_binding_and_bearer_header_normalization() {
    let client = CoreClient::from_provider_metadata(
        serde_json::from_value::<CoreProviderMetadata>(metadata()).unwrap(),
        ClientId::new(CLIENT.into()),
        None,
    );
    let token = sign(claims());
    let verifier = verifier();
    let nonce = Nonce::new("synthetic-nonce".into());
    let verified_subject = token.claims(&verifier, &nonce).unwrap().subject().clone();
    for (body, bind, accepted) in [
        (json!({"sub":"synthetic-person"}), true, true),
        (json!({"sub":"other-person"}), true, false),
        (json!({}), true, false),
        (json!({"sub":"other-person"}), false, true),
    ] {
        let count = RefCell::new(0);
        let http = |request: HttpRequest| {
            *count.borrow_mut() += 1;
            assert_eq!(request.method(), "GET");
            assert_eq!(request.uri().to_string(), format!("{ISSUER}/userinfo"));
            assert!(request.body().is_empty());
            let header = &request.headers()["authorization"];
            assert_eq!(header, "Bearer synthetic-access-token");
            assert!(
                !header.is_sensitive(),
                "pinned default changed; revisit required normalization"
            );
            std::future::ready(Ok::<_, io::Error>(
                openidconnect::http::Response::builder()
                    .header("content-type", "application/json")
                    .body(serde_json::to_vec(&body).unwrap())
                    .unwrap(),
            ))
        };
        let result: Result<CoreUserInfoClaims, _> = ready(
            client
                .user_info(
                    AccessToken::new("synthetic-access-token".into()),
                    bind.then(|| verified_subject.clone()),
                )
                .unwrap()
                .request_async(&http),
        );
        assert_eq!(result.is_ok(), accepted);
        assert_eq!(*count.borrow(), 1);
        // The unbound acceptance case is evidence of a caller obligation, never a Fasti policy.
    }
}

#[test]
fn malformed_userinfo_bearer_panics_before_dispatch_in_pinned_library() {
    let client = CoreClient::from_provider_metadata(
        serde_json::from_value::<CoreProviderMetadata>(metadata()).unwrap(),
        ClientId::new(CLIENT.into()),
        None,
    );
    let count = RefCell::new(0);
    let http = |_: HttpRequest| {
        *count.borrow_mut() += 1;
        std::future::ready(Err::<openidconnect::HttpResponse, _>(io::Error::other(
            "unexpected dispatch",
        )))
    };
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<CoreUserInfoClaims, _> = ready(
            client
                .user_info(
                    AccessToken::new("synthetic\ninvalid-header".into()),
                    Some(SubjectIdentifier::new("synthetic-person".into())),
                )
                .unwrap()
                .request_async(&http),
        );
    }));
    assert!(panic.is_err());
    assert_eq!(*count.borrow(), 0);
    // Qualification evidence only. Production must validate ingress, not catch protocol panics.
}
