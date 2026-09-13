mod tests {
    use super::*;

    fn observed() -> DateTime<Utc> {
        "2026-09-05T12:00:00.123456789Z".parse().unwrap()
    }

    fn seconds(value: u64) -> Duration {
        Duration::from_secs(value)
    }

    fn at(seconds: i64) -> DateTime<Utc> {
        observed() + chrono::Duration::seconds(seconds)
    }

    fn policy(
        reuse: ProviderResponseReuse,
        age: u64,
        fresh: Option<u64>,
        stale_if_error: Option<u64>,
    ) -> ProviderResponseCachePolicy {
        ProviderResponseCachePolicy::new(
            reuse,
            observed(),
            seconds(age),
            fresh.map(seconds),
            stale_if_error.map(seconds),
        )
    }

    #[test]
    fn response_cache_source_age_and_fasti_caps_both_limit_freshness() {
        for (age, source, expected_fresh) in [(20, 90, 70), (10, 1000, 120), (100, 90, 0)] {
            assert_eq!(
                policy(ProviderResponseReuse::Reusable, age, Some(source), None)
                    .deadlines(seconds(120), seconds(600)),
                Some((at(expected_fresh), at(600)))
            );
        }
    }

    #[test]
    fn response_cache_missing_freshness_is_not_explicit_zero() {
        assert_eq!(
            policy(ProviderResponseReuse::Reusable, 1000, None, None)
                .deadlines(seconds(120), seconds(600)),
            Some((at(120), at(600)))
        );
        assert_eq!(
            policy(ProviderResponseReuse::Reusable, 0, Some(0), None)
                .deadlines(seconds(120), seconds(600)),
            Some((at(0), at(600)))
        );
    }

    #[test]
    fn response_cache_stale_deadline_is_absolute_not_added_to_fresh_cap() {
        let reusable = policy(ProviderResponseReuse::Reusable, 0, None, None);
        assert_eq!(
            reusable.deadlines(seconds(120), seconds(600)),
            Some((at(120), at(600)))
        );
        assert_eq!(
            reusable.deadlines(seconds(700), seconds(600)),
            Some((at(600), at(600)))
        );
        assert_eq!(
            reusable.deadlines(seconds(120), Duration::ZERO),
            Some((at(0), at(0)))
        );
    }

    #[test]
    fn response_cache_shorter_stale_if_error_consumes_source_age_and_preserves_zero() {
        for (age, source, grace, expected_fresh, expected_stale) in [
            (20, Some(90), 30, 70, 100),
            (100, Some(90), 30, 0, 20),
            (130, Some(90), 30, 0, 0),
            (20, Some(90), 0, 70, 70),
            (0, None, 30, 120, 150),
            (0, None, 0, 120, 120),
            (0, Some(1000), 1000, 120, 600),
        ] {
            assert_eq!(
                policy(ProviderResponseReuse::Reusable, age, source, Some(grace))
                    .deadlines(seconds(120), seconds(600)),
                Some((at(expected_fresh), at(expected_stale))),
                "age={age} source={source:?} grace={grace}"
            );
        }
    }

    #[test]
    fn response_cache_no_store_is_not_zero_ttl_admission() {
        for (fresh, stale) in [(0, 0), (120, 600)] {
            let observation = policy(ProviderResponseReuse::NoStore, 0, Some(1000), Some(1000));
            assert_eq!(observation.reuse(), ProviderResponseReuse::NoStore);
            assert_eq!(observation.deadlines(seconds(fresh), seconds(stale)), None);
        }
    }

    #[test]
    fn response_cache_validate_every_reuse_has_no_fresh_or_stale_window() {
        for age in [0, 1000] {
            assert_eq!(
                policy(
                    ProviderResponseReuse::ValidateEveryReuse,
                    age,
                    Some(1000),
                    Some(1000)
                )
                .deadlines(seconds(120), seconds(600)),
                Some((observed(), observed()))
            );
        }
    }

    #[test]
    fn response_cache_validate_when_stale_never_uses_stale_if_error() {
        for (age, expected_fresh) in [(20, 70), (100, 0)] {
            assert_eq!(
                policy(
                    ProviderResponseReuse::ValidateWhenStale,
                    age,
                    Some(90),
                    Some(1000)
                )
                .deadlines(seconds(120), seconds(600)),
                Some((at(expected_fresh), at(expected_fresh)))
            );
        }
    }

    #[test]
    fn response_cache_roundtrip_and_delayed_conversion_never_renew_observation_time() {
        for reuse in [
            ProviderResponseReuse::NoStore,
            ProviderResponseReuse::ValidateEveryReuse,
            ProviderResponseReuse::ValidateWhenStale,
            ProviderResponseReuse::Reusable,
        ] {
            let original = policy(reuse, 20, Some(90), Some(30));
            let json = serde_json::to_string(&original).unwrap();
            let restored: ProviderResponseCachePolicy = serde_json::from_str(&json).unwrap();
            assert_eq!(original, restored);
            assert_eq!(restored.received_at(), observed());
            assert_eq!(serde_json::to_string(&restored).unwrap(), json);
            let deadlines = original.deadlines(seconds(120), seconds(600));
            assert_eq!(restored.deadlines(seconds(120), seconds(600)), deadlines);
            // A later consumer may apply another purpose cap, but cannot inject
            // its conversion clock. All deadlines remain relative to observation.
            if let Some((fresh, stale)) = restored.deadlines(seconds(30), seconds(60)) {
                assert!(fresh <= at(30));
                assert!(stale <= at(60));
                assert!(stale < at(3600));
            }
        }
    }

    #[test]
    fn response_cache_serialization_rejects_unknown_fields_and_invalid_required_state() {
        let original =
            serde_json::to_value(policy(ProviderResponseReuse::Reusable, 20, Some(90), None))
                .unwrap();
        for field in ["stored_at", "raw_headers", "permission", "fresh_until"] {
            let mut hostile = original.clone();
            hostile[field] = true.into();
            assert!(
                serde_json::from_value::<ProviderResponseCachePolicy>(hostile).is_err(),
                "{field}"
            );
        }
        for field in ["reuse", "received_at", "corrected_initial_age"] {
            let mut hostile = original.clone();
            hostile.as_object_mut().unwrap().remove(field);
            assert!(
                serde_json::from_value::<ProviderResponseCachePolicy>(hostile).is_err(),
                "missing {field}"
            );
        }
        for (field, value) in [
            ("reuse", serde_json::json!("allow_all")),
            ("received_at", serde_json::json!("not-a-time")),
            ("corrected_initial_age", serde_json::json!(-1)),
        ] {
            let mut hostile = original.clone();
            hostile[field] = value;
            assert!(
                serde_json::from_value::<ProviderResponseCachePolicy>(hostile).is_err(),
                "invalid {field}"
            );
        }
    }

    #[test]
    fn response_cache_canonical_policy_preserves_all_modes_and_duration_extremes() {
        for reuse in [
            ProviderResponseReuse::NoStore,
            ProviderResponseReuse::ValidateEveryReuse,
            ProviderResponseReuse::ValidateWhenStale,
            ProviderResponseReuse::Reusable,
        ] {
            for (age, fresh, stale) in [
                (Duration::ZERO, None, None),
                (Duration::ZERO, Some(Duration::ZERO), Some(Duration::ZERO)),
                (
                    Duration::new(17, 250_000_000),
                    Some(Duration::new(90, 1)),
                    Some(Duration::new(30, 999_999_999)),
                ),
                (Duration::MAX, Some(Duration::MAX), Some(Duration::MAX)),
            ] {
                let original =
                    ProviderResponseCachePolicy::new(reuse, observed(), age, fresh, stale);
                let encoded = original.to_canonical_json();
                assert!(encoded.len() <= ProviderResponseCachePolicy::MAX_JSON_BYTES);
                let decoded = ProviderResponseCachePolicy::from_canonical_json(&encoded)
                    .expect("canonical observation, including saturated upstream age");
                assert_eq!(decoded, original);
                assert_eq!(decoded.to_canonical_json(), encoded);
                assert_eq!(
                    decoded.deadlines(seconds(120), seconds(600)),
                    original.deadlines(seconds(120), seconds(600))
                );
                if reuse == ProviderResponseReuse::NoStore {
                    // Valid representation is not durable payload admission.
                    assert_eq!(decoded.deadlines(seconds(120), seconds(600)), None);
                }
            }
        }
        let absent = policy(ProviderResponseReuse::Reusable, 0, None, None);
        let zero = policy(ProviderResponseReuse::Reusable, 0, Some(0), Some(0));
        assert_ne!(absent.to_canonical_json(), zero.to_canonical_json());
        assert_eq!(
            absent.to_canonical_json(),
            concat!(
                "{\"reuse\":\"reusable\",",
                "\"received_at\":\"2026-09-05T12:00:00.123456789Z\",",
                "\"corrected_initial_age\":{\"secs\":0,\"nanos\":0},",
                "\"source_freshness\":null,\"source_stale_if_error\":null}"
            )
        );
    }

    #[test]
    fn response_cache_canonical_policy_rejects_serde_normalized_representations() {
        let encoded = policy(ProviderResponseReuse::Reusable, 0, None, None).to_canonical_json();
        for alternate in [
            encoded.replace("{\"secs\":0,\"nanos\":0}", "[0,0]"),
            encoded.replace("\"nanos\":0", "\"nanos\":1000000000"),
            encoded.replace(".123456789Z", ".123456789+00:00"),
            encoded.replace(",\"source_freshness\":null", ""),
            encoded.replace(",\"source_stale_if_error\":null", ""),
            encoded.replace("\"reusable\"", "\"reus\\u0061ble\""),
            encoded.replace("{\"secs\":0,\"nanos\":0}", "{\"nanos\":0,\"secs\":0}"),
            format!(" {encoded}"),
            format!("{encoded}\n"),
        ] {
            assert_ne!(alternate, encoded, "fixture must alter persisted bytes");
            assert!(
                serde_json::from_str::<ProviderResponseCachePolicy>(&alternate).is_ok(),
                "valid but noncanonical serde representation: {alternate}"
            );
            assert!(
                ProviderResponseCachePolicy::from_canonical_json(&alternate).is_none(),
                "noncanonical observation: {alternate}"
            );
        }
    }

    #[test]
    fn response_cache_canonical_policy_rejects_duplicate_unknown_and_malformed_state() {
        let encoded = policy(ProviderResponseReuse::Reusable, 0, None, None).to_canonical_json();
        for malformed in [
            encoded.replace("\"reuse\":", "\"reuse\":\"no_store\",\"reuse\":"),
            encoded.replace("\"reuse\":", "\"unknown\":true,\"reuse\":"),
            encoded.replace("\"reusable\"", "\"allow_all\""),
            encoded.replace(
                "\"source_freshness\":null",
                "\"source_freshness\":null,\"source_freshness\":null",
            ),
            encoded.replace(
                "\"source_stale_if_error\":null",
                "\"source_stale_if_error\":null,\"source_stale_if_error\":null",
            ),
            encoded.replace("\"secs\":0", "\"secs\":0,\"secs\":1"),
            encoded.replace("\"nanos\":0", "\"nanos\":0,\"nanos\":1"),
            encoded.replace("\"secs\":0", "\"unknown\":0,\"secs\":0"),
            encoded.replace("\"secs\":0", "\"secs\":-1"),
            encoded.replace("\"secs\":0", "\"secs\":0.5"),
            encoded.replace("\"secs\":0", "\"secs\":18446744073709551616"),
            encoded.replace("\"nanos\":0", "\"nanos\":4294967296"),
            encoded.replace(
                "{\"secs\":0,\"nanos\":0}",
                "{\"secs\":18446744073709551615,\"nanos\":1000000000}",
            ),
            encoded.replace("{\"secs\":0,\"nanos\":0}", "null"),
            encoded.replace("\"secs\":0,", ""),
            encoded.replace(",\"nanos\":0", ""),
            encoded.replace("2026-09-05T12:00:00.123456789Z", "not-a-time"),
            encoded.replace("\"reuse\":\"reusable\",", ""),
            encoded.replace("\"received_at\":\"2026-09-05T12:00:00.123456789Z\",", ""),
            encoded.replace("\"corrected_initial_age\":{\"secs\":0,\"nanos\":0},", ""),
            String::new(),
            "null".to_owned(),
            "{}".to_owned(),
            "[]".to_owned(),
            format!("{encoded}{encoded}"),
        ] {
            assert_ne!(malformed, encoded, "fixture must alter persisted bytes");
            assert!(
                ProviderResponseCachePolicy::from_canonical_json(&malformed).is_none(),
                "malformed observation: {malformed}"
            );
        }
    }

    #[test]
    fn response_cache_canonical_policy_rejects_bounded_and_oversized_padding() {
        let original = policy(ProviderResponseReuse::Reusable, 0, None, None);
        let encoded = original.to_canonical_json();
        for length in [
            ProviderResponseCachePolicy::MAX_JSON_BYTES,
            ProviderResponseCachePolicy::MAX_JSON_BYTES + 1,
        ] {
            let padded = format!("{encoded}{}", " ".repeat(length - encoded.len()));
            assert_eq!(padded.len(), length);
            assert_eq!(
                serde_json::from_str::<ProviderResponseCachePolicy>(&padded).unwrap(),
                original
            );
            assert!(ProviderResponseCachePolicy::from_canonical_json(&padded).is_none());
        }
    }
}
