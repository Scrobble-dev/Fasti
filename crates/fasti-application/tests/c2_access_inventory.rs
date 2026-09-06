//! Pure pagination and browser-evidence checks, not SQL or authorization proof.

use chrono::{DateTime, Utc};
use fasti_application::*;
use fasti_domain::{
    AccessConsentRevisionId, ClientId, PersonalAccessTokenId, RequestCorrelationId,
};
use static_assertions::assert_not_impl_any;
use std::fmt;

assert_not_impl_any!(AccessInventoryQuery<ClientId>:
    fmt::Debug, Clone, Default, serde::Serialize, serde::de::DeserializeOwned,
    From<RequestAccessContext>, From<ApplicationAccessContext>, From<AuthenticatedBrowserSession>,
    From<SecretMaterial>, From<PersonalAccessTokenSecret>, From<BrowserSessionQuery>,
    From<AccessInventoryQuery<PersonalAccessTokenId>>);
assert_not_impl_any!(AccessInventoryQuery<PersonalAccessTokenId>:
    fmt::Debug, Clone, Default, serde::Serialize, serde::de::DeserializeOwned,
    From<RequestAccessContext>, From<ApplicationAccessContext>, From<AuthenticatedBrowserSession>,
    From<SecretMaterial>, From<PersonalAccessTokenSecret>, From<BrowserSessionQuery>,
    From<AccessInventoryQuery<ClientId>>);
assert_not_impl_any!(AccessInventoryQuery<AccessConsentRevisionId>:
    fmt::Debug, Clone, Default, serde::Serialize, serde::de::DeserializeOwned,
    From<RequestAccessContext>, From<ApplicationAccessContext>, From<AuthenticatedBrowserSession>,
    From<SecretMaterial>, From<PersonalAccessTokenSecret>, From<BrowserSessionQuery>,
    From<AccessInventoryQuery<ClientId>>);

fn utc(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .unwrap()
        .with_timezone(&Utc)
}

#[test]
fn page_size_defaults_to_32_and_accepts_exactly_one_through_100() {
    let default = AccessInventoryPage::<ClientId>::try_new(None, None, None).unwrap();
    assert_eq!(default.limit(), 32);
    assert_eq!(default.after(), None);
    for limit in 1..=100 {
        let page = AccessInventoryPage::<ClientId>::try_new(Some(limit), None, None).unwrap();
        assert_eq!(page.limit(), limit);
    }
    for limit in [0, 101, u16::MAX] {
        assert_eq!(
            AccessInventoryPage::<ClientId>::try_new(Some(limit), None, None),
            Err(AccessInventoryInputError::InvalidLimit)
        );
    }
}

fn assert_typed_cursor<Id: Copy + fmt::Debug + Eq>(id: Id) {
    let time = DateTime::<Utc>::UNIX_EPOCH;
    let page = AccessInventoryPage::try_new(None, Some(time), Some(id)).unwrap();
    let cursor: &(DateTime<Utc>, Id) = page.after().unwrap();
    assert_eq!(cursor, &(time, id));
    assert_eq!(
        AccessInventoryPage::<Id>::try_new(None, Some(time), None),
        Err(AccessInventoryInputError::IncompleteCursor)
    );
    assert_eq!(
        AccessInventoryPage::<Id>::try_new(None, None, Some(id)),
        Err(AccessInventoryInputError::IncompleteCursor)
    );
    assert_eq!(
        AccessInventoryPage::<Id>::try_new(None, None, None)
            .unwrap()
            .after(),
        None
    );
}

#[test]
fn cursors_retain_each_endpoint_id_type_and_require_both_fields() {
    assert_typed_cursor(ClientId::new_v7());
    assert_typed_cursor(PersonalAccessTokenId::new_v7());
    assert_typed_cursor(AccessConsentRevisionId::new_v7());
}

#[test]
fn utc_microsecond_truncation_preserves_offsets_negative_instants_and_leap_seconds() {
    let id = ClientId::new_v7();
    for (input, expected) in [
        (
            "2026-09-04T12:30:00.123456789+01:00",
            "2026-09-04T11:30:00.123456Z",
        ),
        (
            "2026-09-04T12:30:00.999999999-03:30",
            "2026-09-04T16:00:00.999999Z",
        ),
        (
            "2026-09-04T12:30:00.123456000Z",
            "2026-09-04T12:30:00.123456Z",
        ),
        (
            "1969-12-31T23:59:59.999999999Z",
            "1969-12-31T23:59:59.999999Z",
        ),
        ("1969-12-31T23:59:59.000000999Z", "1969-12-31T23:59:59Z"),
        (
            "2016-12-31T23:59:60.123456789Z",
            "2016-12-31T23:59:60.123456Z",
        ),
        (
            "2017-01-01T00:59:60.999999999+01:00",
            "2016-12-31T23:59:60.999999Z",
        ),
    ] {
        let page = AccessInventoryPage::try_new(None, Some(utc(input)), Some(id)).unwrap();
        assert_eq!(page.after(), Some(&(utc(expected), id)), "input {input}");
    }
    let leap =
        AccessInventoryPage::try_new(None, Some(utc("2016-12-31T23:59:60Z")), Some(id)).unwrap();
    assert_eq!(
        leap.after().unwrap().0.timestamp_subsec_nanos(),
        1_000_000_000
    );
    assert_ne!(leap.after().unwrap().0, utc("2017-01-01T00:00:00Z"));
}

#[test]
fn extreme_datetime_cursors_truncate_without_overflow() {
    let id = PersonalAccessTokenId::new_v7();
    let min = DateTime::<Utc>::MIN_UTC;
    let max = DateTime::<Utc>::MAX_UTC;
    let expected_max = DateTime::from_timestamp(max.timestamp(), 999_999_000).unwrap();
    for (input, expected) in [(min, min), (max, expected_max)] {
        let page = AccessInventoryPage::try_new(Some(100), Some(input), Some(id)).unwrap();
        assert_eq!(page.after(), Some(&(expected, id)));
    }
}

#[test]
fn inventory_borrows_retained_browser_request_secret_and_page() {
    let boundary = BrowserRequestBoundaryPolicy::try_new("http://127.0.0.1:8420", "127.0.0.1:8420")
        .unwrap()
        .validate_read(Some("127.0.0.1:8420"))
        .unwrap();
    let correlation = RequestCorrelationId::new_v7();
    let received = utc("2026-09-04T11:30:00.123456789Z");
    let cursor_id = AccessConsentRevisionId::new_v7();
    let page = AccessInventoryPage::try_new(Some(17), Some(received), Some(cursor_id)).unwrap();
    let query = AccessInventoryQuery::new(
        BrowserSessionQuery::new(correlation, SecretMaterial::from_bytes([7; 32]), received),
        boundary,
        page,
    );
    let browser = query.browser_request();
    assert_eq!(browser.correlation_id(), correlation);
    assert_eq!(browser.now(), received);
    assert!(browser
        .session_secret()
        .constant_time_eq(&SecretMaterial::from_bytes([7; 32])));
    assert!(std::ptr::eq(browser, query.browser_request()));
    assert!(std::ptr::eq(
        browser.session_secret(),
        query.browser_request().session_secret()
    ));
    assert!(std::ptr::eq(query.page(), query.page()));
    assert_eq!(query.page(), &page);
    assert_eq!(
        query.page().after(),
        Some(&(utc("2026-09-04T11:30:00.123456Z"), cursor_id))
    );
}
