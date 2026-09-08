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
        let picker = runtime
            .search(
                query.clone(),
                &OutboundAccessPolicy::default(),
                &state(SEARCH_CAPABILITY),
            )
            .await
            .unwrap();
        assert!(picker.iter().any(|candidate| candidate.kind == "anime"));
        assert!(picker.iter().any(|candidate| candidate.kind == "manga"));
        assert!(picker
            .iter()
            .all(|candidate| candidate.response_cache_policy().is_some()));
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
                    kind: first.kind.to_owned(),
                    locale: None,
                    region: None,
                },
                &OutboundAccessPolicy::default(),
                &state(READ_CAPABILITY),
            )
            .await
            .unwrap();
        assert_eq!(selected.identifier().unwrap(), first.identifier().unwrap());
        assert_eq!(
            selected.kitsu_manga_subtype.is_some(),
            selected.kind == "manga"
        );
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
        assert!(kitsu::validate_health_response(&body(vec![], None), "manga").is_ok());
        assert!(kitsu::validate_health_response(&body(vec![manga(None)], None), "manga").is_ok());
        let mut wrong_type = manga(None);
        wrong_type["type"] = json!("anime");
        assert!(
            kitsu::validate_health_response(&body(vec![wrong_type.clone()], None), "anime").is_ok()
        );
        assert!(kitsu::validate_health_response(&body(vec![manga(None)], None), "anime").is_err());
        for invalid in [
            b"{}".to_vec(),
            br#"{"errors":[{"status":"503"}]}"#.to_vec(),
            body(vec![wrong_type], None),
            body(vec![manga(None), manga(None)], None),
        ] {
            assert!(kitsu::validate_health_response(&invalid, "manga").is_err());
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
                parse_kitsu_candidates(&body(vec![resource.clone()], None), 1, &query(), "manga")
                    .unwrap();
            let details = parse_kitsu_selection(
                &serde_json::to_vec(&json!({"data": resource})).unwrap(),
                "42",
                "manga",
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

    #[test]
    fn kitsu_full_body_without_next_requires_an_exhaustion_observation() {
        for kind in ["anime", "manga"] {
            let resources: Vec<_> = (1..=10)
                .map(|id| {
                    let mut item = manga(None);
                    item["id"] = json!(id.to_string());
                    item["type"] = json!(kind);
                    item
                })
                .collect();
            let raw = body(resources.clone(), None);
            let page = parse_kitsu_candidates(&raw, 4, &query(), kind).unwrap();
            assert_eq!(page.next_page, Some(5));
            assert_eq!(page.evidence_digest, provider_evidence_digest(&raw));
            assert_eq!(
                parse_kitsu_candidates(&body(vec![], None), 5, &query(), kind)
                    .unwrap()
                    .next_page,
                None
            );
            assert_eq!(
                parse_kitsu_candidates(&body(resources[..7].to_vec(), None), 5, &query(), kind)
                    .unwrap()
                    .next_page,
                None
            );
        }
    }

    #[tokio::test]
    #[ignore = "explicit public Kitsu complete-tail observation; no credentials"]
    async fn kitsu_live_mixed_search_observes_both_source_tails() {
        let vault = Arc::new(CountingVault::default());
        let runtime = ProviderRuntime::new(vault.clone());
        let state = credential_free_state(
            KITSU_PROVIDER,
            SEARCH_CAPABILITY,
            runtime
                .configuration_digest(KITSU_PROVIDER, SEARCH_CAPABILITY)
                .unwrap()
                .as_str(),
        );
        let mut token = Some(1);
        let mut pages = 0;
        let mut identities = BTreeSet::new();
        let mut rows = 0;
        while let Some(current) = token {
            assert!(pages < 16, "bounded live probe did not reach exhaustion");
            let page = runtime
                .search_page(
                    ProviderSearchInput {
                        provider: KITSU_PROVIDER.into(),
                        query: "naruto".into(),
                    },
                    current,
                    None,
                    &OutboundAccessPolicy::default(),
                    &state,
                )
                .await
                .unwrap();
            for candidate in &page.candidates {
                assert!(candidate.response_cache_policy().is_some());
                identities.insert((candidate.kind, candidate.provider_id.clone()));
            }
            rows += page.candidates.len();
            pages += 1;
            token = page.next_page;
            eprintln!(
                "Kitsu token={current} rows={} next={token:?}",
                page.candidates.len()
            );
        }
        for kind in ["anime", "manga"] {
            let count = identities
                .iter()
                .filter(|(resource, _)| *resource == kind)
                .count();
            assert!(
                count > 20,
                "probe must traverse beyond the initial hit window"
            );
            eprintln!("Kitsu {kind} unique={count}");
        }
        eprintln!(
            "Kitsu pages={pages} rows={rows} unique={}",
            identities.len()
        );
        assert_eq!(vault.loads.load(Ordering::Relaxed), 0);
    }

    fn assert_subtype(subtype: Option<Value>, expected: KitsuMangaSubtype) {
        let resource = manga(subtype);
        let search_body = body(vec![resource.clone()], None);
        let page = parse_kitsu_candidates(&search_body, 1, &query(), "manga").unwrap();
        assert_eq!(page.candidates.len(), 1);
        assert_eq!(page.next_page, None);
        let detail_body = serde_json::to_vec(&json!({"data": resource})).unwrap();
        let detail = parse_kitsu_selection(&detail_body, "42", "manga").unwrap();
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
    fn kitsu_anime_search_and_details_keep_typed_identity_without_manga_native_facts() {
        let mut resource = manga(Some(json!("manga")));
        resource["type"] = json!("anime");
        resource["attributes"]["canonicalTitle"] = json!("An Anime");
        let next = kitsu::source_search_url(&query(), 2, "anime").unwrap();
        let search_body = body(vec![resource.clone()], Some(next.to_string()));
        let policy = crate::cache_policy::observe(
            &reqwest::header::HeaderMap::new(),
            chrono::Utc::now(),
            std::time::Duration::ZERO,
        );
        let page = parse_kitsu_candidates(&search_body, 1, &query(), "anime")
            .unwrap()
            .with_response_cache_policy(policy);
        assert_eq!(page.candidates.len(), 1);
        assert_eq!(
            page.next_page,
            Some(2),
            "parser returns the raw source page"
        );
        assert_eq!(page.evidence_digest, provider_evidence_digest(&search_body));
        let detail_body = serde_json::to_vec(&json!({"data": resource})).unwrap();
        let detail = parse_kitsu_selection(&detail_body, "42", "anime")
            .unwrap()
            .with_response_cache_policy(policy);
        assert_eq!(
            page.candidates[0].identifier().unwrap(),
            detail.identifier().unwrap()
        );
        for (candidate, raw) in [
            (&page.candidates[0], search_body.as_slice()),
            (&detail, detail_body.as_slice()),
        ] {
            assert_eq!(candidate.provider, "kitsu");
            assert_eq!(candidate.kind, "anime");
            assert_eq!(candidate.provider_id, "42");
            assert_eq!(candidate.title, "An Anime");
            assert_eq!(candidate.grain().unwrap(), fasti_domain::Grain::Release);
            assert_eq!(candidate.identifier().unwrap().namespace(), "kitsu.anime");
            assert_eq!(candidate.evidence_digest, provider_evidence_digest(raw));
            assert_eq!(candidate.kitsu_manga_subtype, None);
            assert_eq!(
                candidate
                    .search_evidence()
                    .unwrap()
                    .data()
                    .kitsu_manga_subtype,
                None
            );
            let fields = candidate.metadata_fields(None, None).unwrap();
            assert!(!fields.is_empty(), "ordinary metadata remains available");
            assert!(
                fields.iter().all(|field| {
                    field.field_key().as_str() != fasti_application::KITSU_MANGA_SUBTYPE_FIELD_KEY
                }),
                "a manga-like subtype on an Anime resource is not Manga evidence"
            );
        }
        for (requested_id, requested_kind) in [("43", "anime"), ("42", "manga")] {
            assert_eq!(
                parse_kitsu_selection(&detail_body, requested_id, requested_kind)
                    .unwrap_err()
                    .problem_code(),
                ProblemCode::ProviderResponseInvalid
            );
        }
        let manga_body = serde_json::to_vec(&json!({"data": manga(None)})).unwrap();
        assert!(parse_kitsu_selection(&manga_body, "42", "anime").is_err());
        let wrong_source = body(vec![manga(None)], Some(next.to_string()));
        let filtered = parse_kitsu_candidates(&wrong_source, 1, &query(), "anime").unwrap();
        assert!(filtered.candidates.is_empty());
        assert_eq!(filtered.next_page, Some(2));
    }

    #[test]
    fn kitsu_search_urls_bind_encoded_query_and_bounded_offsets() {
        for (page, kind, offset) in [
            (1, "anime", "0"),
            (2, "manga", "0"),
            (5, "anime", "10"),
            (8, "manga", "10"),
        ] {
            let url = search_url("kitsu", &query(), page, None).unwrap();
            assert_eq!(url.scheme(), "https");
            assert_eq!(url.host_str(), Some("kitsu.io"));
            assert_eq!(url.path(), format!("/api/edge/{kind}"));
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
            parse_kitsu_candidates(&duplicate, 1, &query(), "manga")
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
            "manga",
        )
        .unwrap();
        assert_eq!(page.candidates.len(), 1);
        for (resource, requested_id) in [(anime, "42"), (resource, "43")] {
            let detail = serde_json::to_vec(&json!({"data": resource})).unwrap();
            assert_eq!(
                parse_kitsu_selection(&detail, requested_id, "manga")
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
        let next = kitsu::source_search_url(&query(), 2, "manga")
            .unwrap()
            .to_string();
        let raw = body(resources, Some(next));
        let page = parse_kitsu_candidates(&raw, 1, &query(), "manga").unwrap();
        assert!(page.candidates.is_empty());
        assert_eq!(page.next_page, Some(2));
        assert_eq!(page.evidence_digest, provider_evidence_digest(&raw));
    }

    #[test]
    fn kitsu_full_raw_page_without_link_continues_even_when_every_candidate_is_filtered() {
        let resources = (1..=10)
            .map(|id| {
                let mut resource = manga(None);
                resource["id"] = json!(id.to_string());
                resource["attributes"] = json!({"subtype": "manga"});
                resource
            })
            .collect::<Vec<_>>();
        let raw = body(resources.clone(), None);
        let page = parse_kitsu_candidates(&raw, 1, &query(), "manga").unwrap();
        assert!(page.candidates.is_empty());
        assert_eq!(page.next_page, Some(2));
        assert_eq!(page.evidence_digest, provider_evidence_digest(&raw));
        let partial = body(resources.into_iter().take(9).collect(), None);
        let page = parse_kitsu_candidates(&partial, 2, &query(), "manga").unwrap();
        assert!(page.candidates.is_empty());
        assert_eq!(page.next_page, None, "a partial unlinked body terminates");
    }

    #[test]
    fn kitsu_source_continuation_drains_repeated_first_page_and_unlinked_tail() {
        for (source_pages, expected_additions, expected_last_id) in [
            (
                vec![
                    (1..=10).collect::<Vec<_>>(),
                    (1..=10).collect(),
                    (11..=20).collect(),
                    (21..=23).collect(),
                ],
                vec![10, 0, 10, 3],
                23,
            ),
            (
                vec![(1..=10).collect(), (11..=20).collect(), Vec::new()],
                vec![10, 10, 0],
                20,
            ),
        ] {
            let mut next = Some(1);
            let mut visited = Vec::new();
            let mut discovered = std::collections::BTreeSet::new();
            let mut additions = Vec::new();
            while let Some(source_page) = next {
                assert!(
                    visited.len() < source_pages.len(),
                    "source traversal must terminate"
                );
                let ids = source_pages
                    .get((source_page - 1) as usize)
                    .expect("continuation must not skip source offsets");
                let resources = ids
                    .iter()
                    .map(|id| {
                        let mut resource = manga(Some(json!("manga")));
                        resource["id"] = json!(id.to_string());
                        resource
                    })
                    .collect();
                // No next link: the parser must use raw fullness, not accepted additions.
                let raw = body(resources, None);
                let page = parse_kitsu_candidates(&raw, source_page, &query(), "manga").unwrap();
                assert_eq!(page.evidence_digest, provider_evidence_digest(&raw));
                assert_eq!(page.candidates.len(), ids.len());
                let before = discovered.len();
                for candidate in page.candidates {
                    assert_eq!(candidate.evidence_digest, provider_evidence_digest(&raw));
                    discovered.insert((candidate.kind, candidate.provider_id));
                }
                additions.push(discovered.len() - before);
                visited.push(source_page);
                next = page.next_page;
                if let Some(next_page) = next {
                    assert_eq!(next_page, source_page + 1);
                }
            }
            assert_eq!(visited, (1..=source_pages.len() as u32).collect::<Vec<_>>());
            assert_eq!(additions, expected_additions);
            assert_eq!(
                discovered,
                (1..=expected_last_id)
                    .map(|id| ("manga", id.to_string()))
                    .collect::<std::collections::BTreeSet<_>>()
            );
        }
    }

    #[test]
    fn kitsu_continuations_reject_coordinate_changes_and_non_forward_pages() {
        let forward = kitsu::source_search_url(&query(), 3, "manga").unwrap();
        let raw = body(vec![manga(Some(json!("manga")))], Some(forward.to_string()));
        assert_eq!(
            parse_kitsu_candidates(&raw, 2, &query(), "manga")
                .unwrap()
                .next_page,
            Some(3)
        );
        let mut invalid = vec![
            kitsu::source_search_url(&query(), 1, "manga").unwrap(),
            kitsu::source_search_url(&query(), 2, "manga").unwrap(),
            kitsu::source_search_url(&SearchQuery::try_new("Different").unwrap(), 3, "manga")
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
                parse_kitsu_candidates(&raw, 2, &query(), "manga")
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
                let page = parse_kitsu_candidates(&response.body, 1, &query(), "manga")
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
