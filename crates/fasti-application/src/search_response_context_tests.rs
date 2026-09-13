mod response_context_tests {
    use super::*;
    use crate::{ProviderResponseCachePolicy, ProviderResponseReuse};
    use std::time::Duration as StdDuration;

    fn context() -> SearchPageContext {
        SearchProviderQuery::try_new(
            SearchQuery::try_new("Private exact query").unwrap(),
            ProviderId::try_new("tmdb").unwrap(),
            2,
            Some(MetadataLocale::try_new("fr-FR").unwrap()),
            Some(MetadataRegion::try_new("FR").unwrap()),
            vec![Grain::Series, Grain::Film],
        )
        .unwrap()
        .receipt_context()
    }

    fn policy(reuse: ProviderResponseReuse) -> ProviderResponseCachePolicy {
        ProviderResponseCachePolicy::new(
            reuse,
            DateTime::from_timestamp(1_700_000_000, 123_000_000).unwrap(),
            StdDuration::new(17, 250_000_000),
            Some(StdDuration::from_secs(90)),
            Some(StdDuration::from_secs(30)),
        )
    }

    #[test]
    fn response_context_policy_roundtrip_does_not_change_original_context_or_digest() {
        let context = context();
        let original = context.to_json().unwrap();
        let digest = context.digest();
        for reuse in [
            ProviderResponseReuse::NoStore,
            ProviderResponseReuse::ValidateEveryReuse,
            ProviderResponseReuse::ValidateWhenStale,
            ProviderResponseReuse::Reusable,
        ] {
            let policy = policy(reuse);
            let encoded = context.to_response_json(&policy).unwrap();
            assert!(encoded.starts_with(&format!(
                "{},\"response_policy\":",
                original.strip_suffix('}').unwrap()
            )));
            assert!(!encoded.contains("Private exact query"));
            let (decoded, observed) = SearchPageContext::from_response_json(&encoded).unwrap();
            assert_eq!(decoded, context);
            assert_eq!(decoded.digest(), digest);
            assert_eq!(decoded.to_json().unwrap(), original);
            assert_eq!(observed, Some(policy));
            assert_eq!(
                decoded.to_response_json(&observed.unwrap()).unwrap(),
                encoded
            );
            // A policy envelope is not silently accepted as the legacy format.
            assert!(SearchPageContext::from_json(&encoded).is_err());
        }
    }

    #[test]
    fn response_context_legacy_absence_is_not_an_assumed_reusable_policy() {
        let context = context();
        let legacy = context.to_json().unwrap();
        let (decoded, observed) = SearchPageContext::from_response_json(&legacy).unwrap();
        assert_eq!(decoded, context);
        assert_eq!(decoded.to_json().unwrap(), legacy);
        assert_eq!(observed, None);
        let explicit_null = format!(
            "{},\"response_policy\":null}}",
            legacy.strip_suffix('}').unwrap()
        );
        assert!(SearchPageContext::from_response_json(&explicit_null).is_err());
    }

    #[test]
    fn response_context_rejects_duplicate_unknown_and_noncanonical_outer_fields() {
        let encoded = context()
            .to_response_json(&policy(ProviderResponseReuse::Reusable))
            .unwrap();
        let policy_json = serde_json::to_string(&policy(ProviderResponseReuse::Reusable)).unwrap();
        let mutations = [
            encoded.replacen("\"page\":2", "\"page\":2,\"page\":2", 1),
            encoded.replacen(
                "\"response_policy\":",
                &format!("\"response_policy\":{policy_json},\"response_policy\":"),
                1,
            ),
            encoded.replacen("\"provider\":", "\"unexpected\":true,\"provider\":", 1),
            encoded.replacen("\"page\":2", "\"page\":0", 1),
            encoded.replace("fr-fr", "FR-fr"),
            encoded.replace("\"FR\"", "\"fr\""),
            encoded.replace("\"film\",\"series\"", "\"series\",\"film\""),
            encoded.replace("\"film\",\"series\"", "\"film\",\"film\""),
            encoded.replace("\"region\":\"FR\",", ""),
            format!(" {encoded}"),
            format!("{encoded}\n"),
            encoded.replacen("\"page\":2", "\"page\" : 2", 1),
        ];
        for mutated in mutations {
            assert_ne!(mutated, encoded, "fixture must change the envelope");
            assert!(
                SearchPageContext::from_response_json(&mutated).is_err(),
                "{mutated}"
            );
        }
    }

    #[test]
    fn response_context_legacy_decoder_keeps_strict_normalized_validation() {
        let legacy = context().to_json().unwrap();
        for mutated in [
            legacy.replace("fr-fr", "FR-fr"),
            legacy.replace("\"page\":2", "\"page\":2,\"page\":2"),
            legacy.replace("\"provider\":", "\"unexpected\":true,\"provider\":"),
            legacy.replace("\"region\":\"FR\",", ""),
            format!("{legacy} "),
        ] {
            assert!(SearchPageContext::from_json(&mutated).is_err());
            assert!(SearchPageContext::from_response_json(&mutated).is_err());
        }
    }

    #[test]
    fn response_context_rejects_invalid_duplicate_and_noncanonical_policy_fields() {
        let encoded = context()
            .to_response_json(&policy(ProviderResponseReuse::Reusable))
            .unwrap();
        for mutated in [
            encoded.replace("\"reuse\":\"reusable\"", "\"reuse\":\"unknown\""),
            encoded.replace("\"reuse\":", "\"reuse\":\"reusable\",\"reuse\":"),
            encoded.replace("\"reuse\":", "\"unexpected\":true,\"reuse\":"),
            encoded.replace("\"secs\":17", "\"secs\":-1"),
            encoded.replace("\"secs\":17", "\"secs\":18446744073709551616"),
            encoded.replace("\"nanos\":250000000", "\"nanos\":1000000000"),
            encoded.replace("2023-11-14T22:13:20.123Z", "2023-11-14T22:13:20.123+00:00"),
        ] {
            assert_ne!(mutated, encoded, "fixture must change policy JSON");
            assert!(
                SearchPageContext::from_response_json(&mutated).is_err(),
                "{mutated}"
            );
        }
    }

    #[test]
    fn response_context_combined_envelope_is_bounded_not_only_each_component() {
        let context = context();
        let policy = policy(ProviderResponseReuse::Reusable);
        let encoded = context.to_response_json(&policy).unwrap();
        assert!(encoded.len() <= MAX_SEARCH_CONTEXT_BYTES);
        assert!(SearchPageContext::from_response_json(&encoded).is_ok());
        // Both JSON components fit individually, but their combined input must
        // not gain a second 2 KiB allowance. Whitespace is valid JSON syntax;
        // the persisted representation also independently requires canonicality.
        let context_json = context.to_json().unwrap();
        let policy_json = serde_json::to_string(&policy).unwrap();
        let padding = " ".repeat(MAX_SEARCH_CONTEXT_BYTES - encoded.len() + 1);
        let combined = format!(
            "{},\"response_policy\":{padding}{policy_json}}}",
            context_json.strip_suffix('}').unwrap()
        );
        assert_eq!(combined.len(), MAX_SEARCH_CONTEXT_BYTES + 1);
        assert!(context_json.len() < MAX_SEARCH_CONTEXT_BYTES);
        assert!(padding.len() + policy_json.len() < MAX_SEARCH_CONTEXT_BYTES);
        assert!(serde_json::from_str::<serde_json::Value>(&combined).is_ok());
        assert!(SearchPageContext::from_response_json(&combined).is_err());
    }
}
