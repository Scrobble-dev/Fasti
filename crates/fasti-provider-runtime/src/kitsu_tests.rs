mod kitsu_tests {
    use super::*;
    use fasti_application::KitsuMangaSubtype;
    use serde_json::{json, Value};

    #[tokio::test]
    #[ignore = "explicit public Kitsu network check; no credentials or fixture transport"]
    async fn kitsu_live_governed_search_and_details_without_credentials() {
        let vault = Arc::new(CountingVault::default());
        let runtime = ProviderRuntime::new(vault.clone());
        let state = |capability| {
            credential_free_state(
                KITSU_PROVIDER,
                capability,
                runtime
                    .configuration_digest(KITSU_PROVIDER, capability)
                    .unwrap()
                    .as_str(),
            )
        };
        let query = ProviderSearchInput {
            provider: KITSU_PROVIDER.to_owned(),
            query: "berserk".to_owned(),
        };
        let page = runtime
            .search_page(
                query.clone(),
                1,
                None,
                &OutboundAccessPolicy::default(),
                &state(SEARCH_CAPABILITY),
            )
            .await
            .unwrap();
        let first = page
            .candidates
            .first()
            .expect("public search returns a candidate");
        let selected = runtime
            .fetch_selection(
                ProviderSelectionInput {
                    provider: KITSU_PROVIDER.to_owned(),
                    provider_id: first.provider_id.clone(),
                    kind: "manga".to_owned(),
                    locale: None,
                    region: None,
                },
                &OutboundAccessPolicy::default(),
                &state(READ_CAPABILITY),
            )
            .await
            .unwrap();
        assert_eq!(selected.identifier().unwrap(), first.identifier().unwrap());
        assert!(selected.kitsu_manga_subtype.is_some());
        assert!(selected.response_cache_policy().is_some());
        if let Some(next) = page.next_page {
            let next_page = runtime
                .search_page(
                    query,
                    next,
                    None,
                    &OutboundAccessPolicy::default(),
                    &state(SEARCH_CAPABILITY),
                )
                .await
                .unwrap();
            assert!(next_page.response_cache_policy.is_some());
        }
        assert_eq!(vault.loads.load(Ordering::Relaxed), 0);
    }

    fn query() -> SearchQuery {
        SearchQuery::try_new("Blue & white").unwrap()
    }

    #[test]
    fn kitsu_health_requires_a_bounded_typed_resource_response() {
        assert!(kitsu::validate_health_response(&body(vec![], None)).is_ok());
        assert!(kitsu::validate_health_response(&body(vec![manga(None)], None)).is_ok());
        let mut wrong_type = manga(None);
        wrong_type["type"] = json!("anime");
        for invalid in [
            b"{}".to_vec(),
            br#"{"errors":[{"status":"503"}]}"#.to_vec(),
            body(vec![wrong_type], None),
            body(vec![manga(None), manga(None)], None),
        ] {
            assert!(kitsu::validate_health_response(&invalid).is_err());
        }
    }

    #[test]
    fn kitsu_description_precedes_the_deprecated_synopsis_with_bounded_fallback() {
        for (description, expected) in [
            (json!("Current description"), "Current description"),
            (Value::Null, "A description."),
            (json!(""), "A description."),
            (json!("x".repeat(4097)), "A description."),
        ] {
            let mut resource = manga(None);
            resource["attributes"]["description"] = description;
            let search =
                parse_kitsu_candidates(&body(vec![resource.clone()], None), 1, &query()).unwrap();
            let details = parse_kitsu_selection(
                &serde_json::to_vec(&json!({"data": resource})).unwrap(),
                "42",
            )
            .unwrap();
            assert_eq!(search.candidates[0].overview.as_deref(), Some(expected));
            assert_eq!(details.overview.as_deref(), Some(expected));
        }
    }

    fn manga(subtype: Option<Value>) -> Value {
        let mut resource = json!({
            "id": "42",
            "type": "manga",
            "attributes": {
                "canonicalTitle": "A Manga",
                "titles": {"en": "A Manga", "ja_jp": "Original"},
                "synopsis": "A description.",
                "startDate": "2020-04-01",
                "mangaType": "manga"
            }
        });
        if let Some(subtype) = subtype {
            resource["attributes"]["subtype"] = subtype;
        }
        resource
    }

    fn body(resources: Vec<Value>, next: Option<String>) -> Vec<u8> {
        serde_json::to_vec(&json!({"data": resources, "links": {"next": next}})).unwrap()
    }

    fn assert_subtype(subtype: Option<Value>, expected: KitsuMangaSubtype) {
        let resource = manga(subtype);
        let search_body = body(vec![resource.clone()], None);
        let page = parse_kitsu_candidates(&search_body, 1, &query()).unwrap();
        assert_eq!(page.candidates.len(), 1);
        assert_eq!(page.next_page, None);
        let detail_body = serde_json::to_vec(&json!({"data": resource})).unwrap();
        let detail = parse_kitsu_selection(&detail_body, "42").unwrap();
        for (candidate, raw) in [
            (&page.candidates[0], search_body.as_slice()),
            (&detail, detail_body.as_slice()),
        ] {
            assert_eq!(candidate.provider, "kitsu");
            assert_eq!(candidate.kind, "manga");
            assert_eq!(candidate.provider_id, "42");
            assert_eq!(candidate.title, "A Manga");
            assert_eq!(candidate.release_year, Some(2020));
            assert_eq!(candidate.overview.as_deref(), Some("A description."));
            assert_eq!(candidate.kitsu_manga_subtype, Some(expected));
            assert_eq!(candidate.evidence_digest, provider_evidence_digest(raw));
            assert_eq!(candidate.grain().unwrap(), fasti_domain::Grain::Work);
            assert_eq!(
                candidate
                    .search_evidence()
                    .unwrap()
                    .data()
                    .kitsu_manga_subtype,
                Some(expected)
            );
            let public = serde_json::to_value(candidate).unwrap();
            let mut keys = public
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>();
            keys.sort_unstable();
            assert_eq!(
                keys,
                [
                    "authors",
                    "image_url",
                    "kind",
                    "original_title",
                    "overview",
                    "provider",
                    "provider_id",
                    "release_year",
                    "title"
                ]
            );
        }
    }

    #[test]
    fn kitsu_search_and_detail_retain_native_positive_and_negative_subtypes() {
        for subtype in [
            KitsuMangaSubtype::Manga,
            KitsuMangaSubtype::Manhwa,
            KitsuMangaSubtype::Manhua,
            KitsuMangaSubtype::Oneshot,
            KitsuMangaSubtype::Doujin,
            KitsuMangaSubtype::Oel,
            KitsuMangaSubtype::Novel,
        ] {
            assert_subtype(Some(json!(subtype.as_str())), subtype);
        }
    }

    #[test]
    fn kitsu_unknown_subtype_never_falls_back_to_deprecated_manga_type() {
        for subtype in [
            None,
            Some(Value::Null),
            Some(json!("unknown")),
            Some(json!("MANGA")),
            Some(json!("light_novel")),
            Some(json!("")),
            Some(json!(17)),
            Some(json!(true)),
            Some(json!(["manga"])),
            Some(json!({"value": "manga"})),
        ] {
            assert_subtype(subtype, KitsuMangaSubtype::Unknown);
        }
    }

    #[test]
    fn kitsu_search_urls_bind_encoded_query_and_bounded_offsets() {
        for (page, offset) in [(1, "0"), (2, "10"), (3, "20")] {
            let url = search_url("kitsu", &query(), page, None).unwrap();
            assert_eq!(url.scheme(), "https");
            assert_eq!(url.host_str(), Some("kitsu.io"));
            assert_eq!(url.path(), "/api/edge/manga");
            assert_eq!(url.username(), "");
            assert!(url.password().is_none());
            assert!(url.fragment().is_none());
            let pairs = url
                .query_pairs()
                .collect::<std::collections::BTreeMap<_, _>>();
            assert_eq!(url.query_pairs().count(), 3);
            assert_eq!(pairs.len(), 3);
            assert_eq!(pairs.get("filter[text]").unwrap(), "Blue & white");
            assert_eq!(pairs.get("page[limit]").unwrap(), "10");
            assert_eq!(pairs.get("page[offset]").unwrap(), offset);
        }
        assert!(search_url("kitsu", &query(), 0, None).is_err());
        assert!(search_url("kitsu", &query(), u32::MAX, None).is_err());
    }

    #[test]
    fn kitsu_rejects_duplicate_coordinates_and_wrong_detail_resource_or_id() {
        let resource = manga(Some(json!("manga")));
        let duplicate = body(vec![resource.clone(), resource.clone()], None);
        assert_eq!(
            parse_kitsu_candidates(&duplicate, 1, &query())
                .unwrap_err()
                .problem_code(),
            ProblemCode::ProviderResponseInvalid
        );
        let mut anime = resource.clone();
        anime["type"] = json!("anime");
        let page = parse_kitsu_candidates(
            &body(vec![anime.clone(), resource.clone()], None),
            1,
            &query(),
        )
        .unwrap();
        assert_eq!(page.candidates.len(), 1);
        for (resource, requested_id) in [(anime, "42"), (resource, "43")] {
            let detail = serde_json::to_vec(&json!({"data": resource})).unwrap();
            assert_eq!(
                parse_kitsu_selection(&detail, requested_id)
                    .unwrap_err()
                    .problem_code(),
                ProblemCode::ProviderResponseInvalid
            );
        }
    }

    #[test]
    fn kitsu_invalid_candidates_do_not_erase_valid_forward_continuation() {
        let mut resources = Vec::new();
        for id in ["0", "-1", "../42", "", " 42"] {
            let mut invalid = manga(Some(json!("manga")));
            invalid["id"] = json!(id);
            resources.push(invalid);
        }
        let mut missing_title = manga(Some(json!("manga")));
        missing_title["attributes"] = json!({"subtype": "manga"});
        resources.push(missing_title);
        let next = search_url("kitsu", &query(), 2, None).unwrap().to_string();
        let raw = body(resources, Some(next));
        let page = parse_kitsu_candidates(&raw, 1, &query()).unwrap();
        assert!(page.candidates.is_empty());
        assert_eq!(page.next_page, Some(2));
        assert_eq!(page.evidence_digest, provider_evidence_digest(&raw));
    }

    #[test]
    fn kitsu_continuations_reject_coordinate_changes_and_non_forward_pages() {
        let forward = search_url("kitsu", &query(), 3, None).unwrap();
        let raw = body(vec![manga(Some(json!("manga")))], Some(forward.to_string()));
        assert_eq!(
            parse_kitsu_candidates(&raw, 2, &query()).unwrap().next_page,
            Some(3)
        );
        let mut invalid = vec![
            search_url("kitsu", &query(), 1, None).unwrap(),
            search_url("kitsu", &query(), 2, None).unwrap(),
            search_url(
                "kitsu",
                &SearchQuery::try_new("Different").unwrap(),
                3,
                None,
            )
            .unwrap(),
        ];
        let mut url = forward.clone();
        url.set_host(Some("example.com")).unwrap();
        invalid.push(url);
        let mut url = forward.clone();
        url.set_path("/api/edge/anime");
        invalid.push(url);
        let mut url = forward.clone();
        url.query_pairs_mut().append_pair("include", "characters");
        invalid.push(url);
        let mut url = forward;
        url.query_pairs_mut().append_pair("page[limit]", "100");
        invalid.push(url);
        for url in invalid {
            let raw = body(vec![manga(Some(json!("manga")))], Some(url.to_string()));
            assert_eq!(
                parse_kitsu_candidates(&raw, 2, &query())
                    .unwrap_err()
                    .problem_code(),
                ProblemCode::ProviderResponseInvalid,
                "{url}"
            );
        }
    }

    fn credential_free_state(
        provider: &str,
        capability: &str,
        digest: &str,
    ) -> ProviderCapabilityState {
        ProviderCapabilityState::try_new(
            ProviderId::try_new(provider).unwrap(),
            ProviderCapabilityId::try_new(capability).unwrap(),
            ProviderCapabilityStatus::Available,
            1,
            CredentialRequirement::None,
            None,
            ProviderCredentialStatus::NotRequired,
            ConfigurationDigest::parse(digest.to_owned()).unwrap(),
            ProviderCheckMetadata::never_run(),
            ProviderCheckMetadata::never_run(),
        )
        .unwrap()
    }

    #[test]
    fn kitsu_credential_free_loading_keeps_configuration_binding_without_vault_reads() {
        let vault = Arc::new(CountingVault::default());
        let runtime = ProviderRuntime::new(vault.clone());
        for capability in [SEARCH_CAPABILITY, READ_CAPABILITY] {
            let client =
                crate::transport::test_authorized_client(KITSU_PROVIDER, capability, KITSU_URL);
            let state =
                credential_free_state(KITSU_PROVIDER, capability, client.configuration_digest());
            validate_state(KITSU_SPEC, capability, &state).unwrap();
            assert!(runtime
                .load_bound_credential(&client, KITSU_SPEC, &state)
                .unwrap()
                .is_none());
            let changed = credential_free_state(KITSU_PROVIDER, capability, &"ab".repeat(32));
            let error = runtime
                .load_bound_credential(&client, KITSU_SPEC, &changed)
                .unwrap_err();
            assert_eq!(error.problem_code(), ProblemCode::ProviderRouteUnavailable);
            assert!(error.detail().contains("configuration changed"));
        }
        assert_eq!(vault.loads.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn kitsu_credential_free_requests_keep_headers_origin_and_secret_rejection() {
        let client =
            crate::transport::test_authorized_client(KITSU_PROVIDER, SEARCH_CAPABILITY, KITSU_URL);
        let url = search_url(KITSU_PROVIDER, &query(), 1, None).unwrap();
        let request = credential_request(KITSU_PROVIDER, &client, url.clone(), None)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(request.url(), &url);
        assert_eq!(request.method(), reqwest::Method::GET);
        assert_eq!(
            request.headers().get(reqwest::header::ACCEPT).unwrap(),
            "application/vnd.api+json"
        );
        assert_eq!(
            request.headers().get(CONTENT_TYPE).unwrap(),
            "application/vnd.api+json"
        );
        assert!(!request.headers().contains_key(AUTHORIZATION));
        assert!(!request.headers().contains_key("x-goog-api-key"));
        let secret = CredentialSecret::try_from_bytes(b"fixture-secret".to_vec()).unwrap();
        assert_eq!(
            credential_request(KITSU_PROVIDER, &client, url.clone(), Some(&secret))
                .unwrap_err()
                .problem_code(),
            ProblemCode::ProviderRouteUnavailable
        );
        let mut foreign = url;
        foreign.set_host(Some("example.com")).unwrap();
        let error = credential_request(KITSU_PROVIDER, &client, foreign, None).unwrap_err();
        assert!(error.detail().contains("origin changed"));
    }

    #[test]
    fn credential_free_state_does_not_authorize_credential_required_providers() {
        let vault = Arc::new(CountingVault::default());
        let runtime = ProviderRuntime::new(vault.clone());
        for spec in [GOOGLE_BOOKS_SPEC, TMDB_SPEC] {
            let (_, url) = endpoint(spec.provider, SEARCH_CAPABILITY).unwrap();
            let client = crate::transport::test_authorized_client(
                spec.provider,
                SEARCH_CAPABILITY,
                url.as_str(),
            );
            let wrong_requirement = credential_free_state(
                spec.provider,
                SEARCH_CAPABILITY,
                client.configuration_digest(),
            );
            assert_eq!(
                validate_state(spec, SEARCH_CAPABILITY, &wrong_requirement)
                    .unwrap_err()
                    .problem_code(),
                ProblemCode::ProviderRouteUnavailable
            );
            assert_eq!(
                credential_request(spec.provider, &client, url, None)
                    .unwrap_err()
                    .problem_code(),
                ProblemCode::ProviderCredentialMissing
            );
            let requirement = spec
                .capabilities
                .iter()
                .find(|entry| entry.capability_id == SEARCH_CAPABILITY)
                .unwrap()
                .credential_requirement;
            let missing = ProviderCapabilityState::try_new(
                ProviderId::try_new(spec.provider).unwrap(),
                ProviderCapabilityId::try_new(SEARCH_CAPABILITY).unwrap(),
                ProviderCapabilityStatus::Available,
                1,
                requirement,
                None,
                ProviderCredentialStatus::Missing,
                ConfigurationDigest::parse(client.configuration_digest().to_owned()).unwrap(),
                ProviderCheckMetadata::never_run(),
                ProviderCheckMetadata::never_run(),
            )
            .unwrap();
            assert_eq!(
                validate_state(spec, SEARCH_CAPABILITY, &missing)
                    .unwrap_err()
                    .problem_code(),
                ProblemCode::ProviderCredentialMissing
            );
            assert_eq!(
                runtime
                    .load_bound_credential(&client, spec, &missing)
                    .unwrap_err()
                    .problem_code(),
                ProblemCode::ProviderCredentialMissing
            );
        }
        assert_eq!(vault.loads.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn kitsu_transport_accepts_json_api_but_not_json_prefix_impostors() {
        // Reuses the actual HTTP reader fixture; this does not bypass or prove
        // governed DNS/TLS authorization, which has separate transport tests.
        for (content_type, accepted) in [
            ("application/vnd.api+json", true),
            ("Application/Vnd.Api+Json; charset=utf-8", true),
            ("application/json", true),
            ("application/json-evil", false),
            ("application/vnd.api+json-evil", false),
            ("text/plain", false),
        ] {
            let body = br#"{"data":[],"links":{"next":null}}"#;
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let (request, server) =
                super::cache_transport_tests::fixture(headers, body.to_vec(), None).await;
            let result = send_json(request, KITSU_SPEC).await;
            server.await.unwrap();
            if accepted {
                let response = result.unwrap();
                assert_eq!(response.body, body);
                assert_eq!(
                    response.cache_policy.reuse(),
                    fasti_application::ProviderResponseReuse::NoStore
                );
                let page = parse_kitsu_candidates(&response.body, 1, &query())
                    .unwrap()
                    .with_response_cache_policy(response.cache_policy);
                assert_eq!(page.response_cache_policy(), Some(&response.cache_policy));
                assert!(page.candidates.is_empty());
            } else {
                assert_eq!(
                    result.err().unwrap().problem_code(),
                    ProblemCode::ProviderResponseInvalid,
                    "{content_type}"
                );
            }
        }
    }
}
