mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    fn received() -> DateTime<Utc> {
        "1994-11-06T08:49:57Z".parse().unwrap()
    }

    fn headers(lines: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in lines {
            headers.append(
                reqwest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        headers
    }

    fn policy(lines: &[(&str, &str)]) -> ProviderResponseCachePolicy {
        observe(&headers(lines), received(), Duration::ZERO)
    }

    fn seconds(policy: ProviderResponseCachePolicy) -> Option<(i64, i64)> {
        policy
            .deadlines(Duration::from_secs(120), Duration::from_secs(600))
            .map(|(fresh, stale)| {
                (
                    (fresh - received()).num_seconds(),
                    (stale - received()).num_seconds(),
                )
            })
    }

    #[test]
    fn absent_headers_keep_fasti_caps_and_original_observation() {
        let value = policy(&[]);
        assert_eq!(value.reuse(), ProviderResponseReuse::Reusable);
        assert_eq!(value.received_at(), received());
        assert_eq!(seconds(value), Some((120, 600)));
    }

    #[test]
    fn long_age_saturates_and_a_long_later_member_does_not_hide_the_first() {
        for length in [128, 129, MAX_POLICY_BYTES] {
            let age = "9".repeat(length);
            assert_eq!(
                seconds(policy(&[
                    ("cache-control", "max-age=60, stale-if-error=20"),
                    ("age", &age)
                ])),
                Some((0, 0))
            );
        }
        let age = format!("10, {}", "9".repeat(MAX_POLICY_BYTES + 1));
        assert_eq!(
            seconds(policy(&[
                ("cache-control", "max-age=60, stale-if-error=20"),
                ("age", &age)
            ])),
            Some((50, 70))
        );
        let oversized = "9".repeat(MAX_POLICY_BYTES + 1);
        let value = policy(&[("cache-control", "max-age=60"), ("age", &oversized)]);
        assert_eq!(value.reuse(), ProviderResponseReuse::ValidateEveryReuse);
        assert_eq!(seconds(value), Some((0, 0)));

        let mut value = headers(&[("cache-control", "max-age=5, stale-if-error=0")]);
        value.insert(AGE, HeaderValue::from_bytes(b"10,\xff").unwrap());
        assert_eq!(
            seconds(observe(&value, received(), Duration::ZERO)),
            Some((0, 0))
        );
        value.insert(AGE, HeaderValue::from_bytes(b"\xff,10").unwrap());
        assert_eq!(
            seconds(observe(&value, received(), Duration::ZERO)),
            Some((5, 5))
        );
    }

    #[test]
    fn unproven_vary_matches_require_validation_and_never_override_no_store() {
        for vary in [
            "*",
            "Accept-Encoding",
            "Origin, Accept-Language",
            "invalid=variant",
        ] {
            let value = policy(&[("cache-control", "max-age=60"), ("vary", vary)]);
            assert_eq!(value.reuse(), ProviderResponseReuse::ValidateEveryReuse);
            assert_eq!(seconds(value), Some((0, 0)));
        }
        assert_eq!(
            policy(&[("vary", " "), ("vary", "*"), ("cache-control", "no-store")]).reuse(),
            ProviderResponseReuse::NoStore
        );
        assert_eq!(seconds(policy(&[("vary", " \t")])), Some((120, 600)));
    }

    #[test]
    fn quoted_numeric_escapes_and_unknown_quoted_commas_parse_without_splitting() {
        for control in [
            "max-age=60, stale-if-error=20",
            "MaX-aGe = \"60\", STALE-IF-ERROR = \"20\"",
            "extension=\"comma, escaped \\\"quote\\\"\", max-age=60, stale-if-error=20",
            "max-age=\"\\6\\0\", stale-if-error=20",
            ", ,\tmax-age=60 , stale-if-error=20,",
        ] {
            let value = policy(&[("cache-control", control)]);
            assert_eq!(value.reuse(), ProviderResponseReuse::Reusable, "{control}");
            assert_eq!(seconds(value), Some((60, 80)), "{control}");
        }
    }

    #[test]
    fn every_cache_control_header_line_contributes_restrictions() {
        assert_eq!(
            seconds(policy(&[
                ("cache-control", "max-age=60"),
                ("cache-control", "stale-if-error=20"),
            ])),
            Some((60, 80))
        );
        let value = policy(&[
            ("cache-control", "max-age=60"),
            ("cache-control", "no-cache"),
            ("cache-control", "no-store"),
        ]);
        assert_eq!(value.reuse(), ProviderResponseReuse::NoStore);
        assert_eq!(seconds(value), None);
    }

    #[test]
    fn no_store_wins_regardless_of_order_or_later_malformed_content() {
        for control in [
            "no-store, max-age=600, stale-if-error=600",
            "max-age=600, no-cache, must-revalidate, no-store",
            "no-store, broken=\"unterminated",
            "broken=\"unterminated, no-store",
        ] {
            let value = policy(&[("cache-control", control)]);
            assert_eq!(value.reuse(), ProviderResponseReuse::NoStore, "{control}");
            assert_eq!(seconds(value), None);
        }
    }

    #[test]
    fn no_cache_including_qualified_form_requires_every_reuse_to_validate() {
        for control in [
            "no-cache, max-age=600, stale-if-error=600",
            "max-age=600, no-cache=\"Date,ETag\"",
            "must-revalidate, no-cache",
        ] {
            let value = policy(&[("cache-control", control)]);
            assert_eq!(value.reuse(), ProviderResponseReuse::ValidateEveryReuse);
            assert_eq!(seconds(value), Some((0, 0)));
        }
    }

    #[test]
    fn must_revalidate_disables_stale_grace_even_when_explicitly_supplied() {
        let value = policy(&[(
            "cache-control",
            "max-age=30, must-revalidate, stale-if-error=600",
        )]);
        assert_eq!(value.reuse(), ProviderResponseReuse::ValidateWhenStale);
        assert_eq!(seconds(value), Some((30, 30)));
    }

    #[test]
    fn duplicate_freshness_even_equal_requires_validation() {
        for lines in [
            vec![("cache-control", "max-age=30, max-age=60")],
            vec![("cache-control", "max-age=30, max-age=30")],
            vec![
                ("cache-control", "max-age=30"),
                ("cache-control", "MAX-AGE=60"),
            ],
        ] {
            let value = policy(&lines);
            assert_eq!(value.reuse(), ProviderResponseReuse::ValidateEveryReuse);
            assert_eq!(seconds(value), Some((0, 0)));
        }
    }

    #[test]
    fn invalid_numeric_freshness_never_falls_back_to_fresh_default() {
        for control in [
            "max-age",
            "max-age=-1",
            "max-age=+1",
            "max-age=1.5",
            "max-age=\"\"",
            "max-age=\" 60 \"",
        ] {
            let value = policy(&[("cache-control", control)]);
            assert_eq!(
                value.reuse(),
                ProviderResponseReuse::ValidateEveryReuse,
                "{control}"
            );
            assert_eq!(seconds(value), Some((0, 0)), "{control}");
        }
    }

    #[test]
    fn malformed_overall_syntax_selects_live_only_no_store_policy() {
        for control in [
            "max-age=",
            "max-age=60; no-store",
            "max-age=\"60",
            "max-age=\"60\"garbage",
            "=60",
            "extension=\"trailing\\",
        ] {
            let value = policy(&[("cache-control", control)]);
            assert_eq!(value.reuse(), ProviderResponseReuse::NoStore, "{control}");
            assert_eq!(seconds(value), None);
        }
    }

    #[test]
    fn duplicate_or_invalid_stale_grace_does_not_grant_default_600_seconds() {
        for suffix in [
            "stale-if-error",
            "stale-if-error=-1",
            "stale-if-error=\"bad\"",
            "stale-if-error=30, stale-if-error=60",
            "stale-if-error=30, stale-if-error=60, stale-if-error=90",
        ] {
            let control = format!("max-age=30, {suffix}");
            assert_eq!(
                seconds(policy(&[("cache-control", &control)])),
                Some((30, 30)),
                "{control}"
            );
        }
    }

    #[test]
    fn overflowing_delta_seconds_saturate_without_wrapping_or_extending_caps() {
        assert_eq!(
            delta_seconds("184467440737095516160"),
            Some(Duration::from_secs(u64::MAX))
        );
        assert_eq!(
            seconds(policy(&[(
                "cache-control",
                "max-age=184467440737095516160, stale-if-error=184467440737095516160"
            )])),
            Some((120, 600))
        );
        assert_eq!(
            seconds(policy(&[
                ("cache-control", "max-age=120"),
                ("age", "184467440737095516160")
            ])),
            Some((0, 600))
        );
    }

    #[test]
    fn cache_control_byte_and_directive_bounds_apply_across_all_lines() {
        let exactly = "x".repeat(MAX_POLICY_BYTES);
        assert_eq!(
            policy(&[("cache-control", &exactly)]).reuse(),
            ProviderResponseReuse::Reusable
        );
        let over = "x".repeat(MAX_POLICY_BYTES + 1);
        assert_eq!(
            policy(&[("cache-control", &over)]).reuse(),
            ProviderResponseReuse::NoStore
        );
        let half = "x".repeat(MAX_POLICY_BYTES / 2 + 1);
        assert_eq!(
            policy(&[("cache-control", &half), ("cache-control", &half)]).reuse(),
            ProviderResponseReuse::NoStore
        );
        let exactly = vec!["extension"; MAX_DIRECTIVES].join(",");
        assert_eq!(
            policy(&[("cache-control", &exactly)]).reuse(),
            ProviderResponseReuse::Reusable
        );
        assert_eq!(
            policy(&[("cache-control", &exactly), ("cache-control", "extension")]).reuse(),
            ProviderResponseReuse::NoStore
        );
    }

    #[test]
    fn non_text_cache_control_cannot_hide_a_storage_restriction() {
        let mut values = HeaderMap::new();
        values.insert(
            CACHE_CONTROL,
            HeaderValue::from_bytes(b"extension=\xff").unwrap(),
        );
        assert_eq!(
            observe(&values, received(), Duration::ZERO).reuse(),
            ProviderResponseReuse::NoStore
        );
    }

    #[test]
    fn all_three_http_date_formats_have_identical_age_and_expiry_meaning() {
        for date in [
            "Sun, 06 Nov 1994 08:49:37 GMT",
            "Sunday, 06-Nov-94 08:49:37 GMT",
            "Sun Nov  6 08:49:37 1994",
        ] {
            assert_eq!(
                seconds(policy(&[("cache-control", "max-age=60"), ("date", date)])),
                Some((40, 600)),
                "{date}"
            );
        }
        for expires in [
            "Sun, 06 Nov 1994 08:50:37 GMT",
            "Sunday, 06-Nov-94 08:50:37 GMT",
            "Sun Nov  6 08:50:37 1994",
        ] {
            assert_eq!(
                seconds(policy(&[
                    ("date", "Sun, 06 Nov 1994 08:49:37 GMT"),
                    ("expires", expires)
                ])),
                Some((40, 600)),
                "{expires}"
            );
        }
    }

    #[test]
    fn expires_without_date_uses_receipt_time_and_invalid_dates_do_not_grant_freshness() {
        assert_eq!(
            seconds(policy(&[("expires", "Sun, 06 Nov 1994 08:50:37 GMT")])),
            Some((40, 600))
        );
        for expires in ["0", "not a date", "Sun, 31 Feb 1994 08:50:37 GMT"] {
            let value = policy(&[("expires", expires)]);
            assert_eq!(value.reuse(), ProviderResponseReuse::ValidateEveryReuse);
            assert_eq!(seconds(value), Some((0, 0)));
        }
        assert_eq!(
            seconds(policy(&[("expires", "Sun, 06 Nov 1994 08:49:37 GMT")])),
            Some((0, 600))
        );
    }

    #[test]
    fn max_age_overrides_expires_even_when_expires_is_invalid_or_duplicated() {
        assert_eq!(
            seconds(policy(&[
                ("cache-control", "max-age=30"),
                ("expires", "0"),
                ("expires", "invalid")
            ])),
            Some((30, 600))
        );
        let value = policy(&[
            ("expires", "Sun, 06 Nov 1994 08:50:37 GMT"),
            ("expires", "Sun, 06 Nov 1994 08:50:37 GMT"),
        ]);
        assert_eq!(value.reuse(), ProviderResponseReuse::ValidateEveryReuse);
    }

    #[test]
    fn age_uses_first_list_member_and_ignores_invalid_values_without_erasing_date_age() {
        assert_eq!(
            seconds(policy(&[
                ("cache-control", "max-age=60"),
                ("age", "10, 50")
            ])),
            Some((50, 600))
        );
        assert_eq!(
            seconds(policy(&[
                ("cache-control", "max-age=60"),
                ("age", "10"),
                ("age", "50")
            ])),
            Some((50, 600))
        );
        for age in ["invalid", "-1", "+1", "1.5", "", "\"10\"", "invalid, 50"] {
            assert_eq!(
                seconds(policy(&[
                    ("cache-control", "max-age=60"),
                    ("date", "Sun, 06 Nov 1994 08:49:37 GMT"),
                    ("age", age)
                ])),
                Some((40, 600)),
                "{age}"
            );
        }
    }

    #[test]
    fn corrected_age_accounts_for_request_delay_and_date_age_without_double_counting() {
        let values = headers(&[
            ("cache-control", "max-age=60, stale-if-error=20"),
            ("date", "Sun, 06 Nov 1994 08:49:37 GMT"),
            ("age", "15"),
        ]);
        assert_eq!(
            seconds(observe(&values, received(), Duration::from_secs(10))),
            Some((35, 55))
        );
        assert_eq!(
            seconds(observe(&values, received(), Duration::from_secs(2))),
            Some((40, 60))
        );
        let future = headers(&[
            ("cache-control", "max-age=60"),
            ("date", "Sun, 06 Nov 1994 08:50:37 GMT"),
        ]);
        assert_eq!(
            seconds(observe(&future, received(), Duration::from_secs(5))),
            Some((55, 600))
        );
    }

    #[test]
    fn absolute_deadlines_preserve_fractional_delay_and_consume_later_residence() {
        let values = headers(&[
            ("cache-control", "max-age=60, stale-if-error=20"),
            ("age", "10"),
        ]);
        let value = observe(&values, received(), Duration::from_millis(1500));
        let (fresh, stale) = value
            .deadlines(Duration::from_secs(120), Duration::from_secs(600))
            .unwrap();
        assert_eq!((fresh - received()).num_milliseconds(), 48_500);
        assert_eq!((stale - received()).num_milliseconds(), 68_500);
        let delayed_commit = received() + chrono::Duration::seconds(20);
        assert_eq!((fresh - delayed_commit).num_milliseconds(), 28_500);
        assert_eq!(value.received_at(), received());
    }

    #[test]
    fn source_freshness_and_stale_grace_can_shorten_but_never_extend_absolute_caps() {
        for (control, age, expected) in [
            ("max-age=30, stale-if-error=20", "20", (10, 30)),
            ("max-age=30, stale-if-error=0", "0", (30, 30)),
            ("max-age=120, stale-if-error=600", "0", (120, 600)),
            ("max-age=3600, stale-if-error=600", "3500", (100, 600)),
            ("max-age=30, stale-if-error=20", "50", (0, 0)),
        ] {
            assert_eq!(
                seconds(policy(&[("cache-control", control), ("age", age)])),
                Some(expected),
                "{control}, Age={age}"
            );
        }
    }
}
