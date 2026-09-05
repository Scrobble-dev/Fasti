//! Bounded HTTP response-policy parsing. Raw headers never leave this adapter.
//! RFC 9111 sections 4.2, 5.1 and 5.2; RFC 5861 section 4.

use chrono::{DateTime, Utc};
use fasti_application::{ProviderResponseCachePolicy, ProviderResponseReuse};
use reqwest::header::{HeaderMap, AGE, CACHE_CONTROL, DATE, EXPIRES, VARY};
use std::time::Duration;

const MAX_POLICY_BYTES: usize = 16 * 1024;
const MAX_DIRECTIVES: usize = 128;

#[cfg(test)]
include!("cache_policy_tests.rs");

pub(crate) fn observe(
    headers: &HeaderMap,
    received_at: DateTime<Utc>,
    request_delay: Duration,
) -> ProviderResponseCachePolicy {
    let mut no_store = false;
    let mut no_cache = false;
    let mut must_revalidate = false;
    let mut freshness = None;
    let mut max_age_seen = false;
    let mut stale_if_error = None;
    let mut stale_if_error_seen = false;
    let mut bytes = 0usize;
    let mut count = 0usize;
    for header in headers.get_all(CACHE_CONTROL) {
        bytes = bytes.saturating_add(header.as_bytes().len());
        let parsed = header.to_str().ok().filter(|_| bytes <= MAX_POLICY_BYTES);
        let Some(value) = parsed else {
            no_store = true;
            break;
        };
        let valid = directives(value, |name, argument| {
            count += 1;
            if count > MAX_DIRECTIVES {
                return Err(());
            }
            if name.eq_ignore_ascii_case("no-store") {
                no_store = true;
            } else if name.eq_ignore_ascii_case("no-cache") {
                // Qualified no-cache is conservatively applied to the whole
                // normalized response, not just its named HTTP headers.
                no_cache = true;
            } else if name.eq_ignore_ascii_case("must-revalidate") {
                must_revalidate = true;
            } else if name.eq_ignore_ascii_case("max-age") {
                let seconds = argument.as_deref().and_then(delta_seconds);
                if max_age_seen || seconds.is_none() {
                    no_cache = true;
                }
                max_age_seen = true;
                freshness = seconds;
            } else if name.eq_ignore_ascii_case("stale-if-error") {
                let seconds = argument.as_deref().and_then(delta_seconds);
                stale_if_error = Some(if stale_if_error_seen {
                    Duration::ZERO
                } else {
                    seconds.unwrap_or(Duration::ZERO)
                });
                stale_if_error_seen = true;
            }
            Ok(())
        });
        // Unparseable syntax may conceal a restriction. Still permit the live
        // response, but never normalize that uncertainty into reusable storage.
        if valid.is_err() {
            no_store = true;
            break;
        }
    }

    let date = single_date(headers, DATE).ok().flatten();
    if !max_age_seen {
        match single_date(headers, EXPIRES) {
            Ok(Some(expires)) => {
                freshness = Some(
                    expires
                        .signed_duration_since(date.unwrap_or(received_at))
                        .to_std()
                        .unwrap_or(Duration::ZERO),
                );
            }
            Ok(None) => {}
            Err(()) => no_cache = true,
        }
    }
    // RFC 9111 5.1: take the first Age list member; ignore an invalid Age.
    // A valid Date-derived apparent age remains authoritative either way.
    let age_value = headers
        .get(AGE)
        .and_then(|value| value.as_bytes().split(|byte| *byte == b',').next())
        .and_then(|value| std::str::from_utf8(value).ok())
        .map(|value| value.trim_matches([' ', '\t']));
    let age = match age_value {
        Some(value) if value.len() > MAX_POLICY_BYTES => {
            // An oversized valid Age must not become an age of zero. Bound
            // numeric work without granting reuse when it exceeds our limit.
            no_cache = true;
            Duration::ZERO
        }
        value => value.and_then(delta_seconds).unwrap_or(Duration::ZERO),
    };
    // We do not retain request-header variants. Even a fresh response cannot
    // be reused without proving its Vary match; '*' can never match (9111 4.1).
    if headers
        .get_all(VARY)
        .iter()
        .any(|value| value.as_bytes().iter().any(|byte| !b" \t".contains(byte)))
    {
        no_cache = true;
    }
    let apparent_age = date
        .and_then(|date| received_at.signed_duration_since(date).to_std().ok())
        .unwrap_or(Duration::ZERO);
    let initial_age = apparent_age.max(age.saturating_add(request_delay));
    let reuse = if no_store {
        ProviderResponseReuse::NoStore
    } else if no_cache {
        ProviderResponseReuse::ValidateEveryReuse
    } else if must_revalidate {
        ProviderResponseReuse::ValidateWhenStale
    } else {
        ProviderResponseReuse::Reusable
    };
    ProviderResponseCachePolicy::new(reuse, received_at, initial_age, freshness, stale_if_error)
}

fn single_date(
    headers: &HeaderMap,
    name: reqwest::header::HeaderName,
) -> Result<Option<DateTime<Utc>>, ()> {
    let mut values = headers.get_all(name).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() || value.as_bytes().len() > 128 {
        return Err(());
    }
    let date = httpdate::parse_http_date(value.to_str().map_err(|_| ())?).map_err(|_| ())?;
    Ok(Some(DateTime::<Utc>::from(date)))
}

fn delta_seconds(value: &str) -> Option<Duration> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    // Overflow means effectively infinite, never wrapped to a short lifetime.
    Some(Duration::from_secs(value.bytes().fold(
        0u64,
        |total, byte| {
            total
                .saturating_mul(10)
                .saturating_add(u64::from(byte - b'0'))
        },
    )))
}

fn token(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn directives(
    value: &str,
    mut visit: impl FnMut(&str, Option<String>) -> Result<(), ()>,
) -> Result<(), ()> {
    let bytes = value.as_bytes();
    let mut position = 0;
    while position < bytes.len() {
        while bytes
            .get(position)
            .is_some_and(|byte| b" \t,".contains(byte))
        {
            position += 1;
        }
        if position == bytes.len() {
            break;
        }
        let start = position;
        while bytes.get(position).is_some_and(|byte| token(*byte)) {
            position += 1;
        }
        if position == start {
            return Err(());
        }
        let name = &value[start..position];
        while bytes
            .get(position)
            .is_some_and(|byte| b" \t".contains(byte))
        {
            position += 1;
        }
        let argument = if bytes.get(position) == Some(&b'=') {
            position += 1;
            while bytes
                .get(position)
                .is_some_and(|byte| b" \t".contains(byte))
            {
                position += 1;
            }
            if bytes.get(position) == Some(&b'"') {
                position += 1;
                let mut decoded = String::new();
                loop {
                    let byte = *bytes.get(position).ok_or(())?;
                    position += 1;
                    match byte {
                        b'"' => break,
                        b'\\' => {
                            let escaped = *bytes.get(position).ok_or(())?;
                            if escaped != b'\t' && !(32..=126).contains(&escaped) {
                                return Err(());
                            }
                            decoded.push(char::from(escaped));
                            position += 1;
                        }
                        b'\t' | 32..=126 => decoded.push(char::from(byte)),
                        _ => return Err(()),
                    }
                }
                Some(decoded)
            } else {
                let start = position;
                while bytes.get(position).is_some_and(|byte| token(*byte)) {
                    position += 1;
                }
                if position == start {
                    return Err(());
                }
                Some(value[start..position].to_owned())
            }
        } else {
            None
        };
        while bytes
            .get(position)
            .is_some_and(|byte| b" \t".contains(byte))
        {
            position += 1;
        }
        if position < bytes.len() && bytes[position] != b',' {
            return Err(());
        }
        visit(name, argument)?;
    }
    Ok(())
}
