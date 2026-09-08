mod search_kitsu_tests {
    use super::*;
    use crate::{
        provider_candidate_metadata_fields, valid_provider_native_fact,
        CreateProviderRecordCommand, GoogleBooksPrintType, KitsuMangaSubtype,
        ProviderResponseCachePolicy, ProviderResponseReuse, RequestAccessContext,
        KITSU_MANGA_SUBTYPE_FIELD_KEY, KITSU_PROVIDER_ID,
    };
    use fasti_domain::{
        CredentialId, FieldClaim, FieldClaimProvenance, FieldClaimStatus, FieldKey,
        MetadataClaimId, MetadataProviderId, NamespaceKey, ReceivedAt, TITLE_FIELD_KEY,
    };

    const VALUES: [(KitsuMangaSubtype, &str); 8] = [
        (KitsuMangaSubtype::Manga, "manga"),
        (KitsuMangaSubtype::Manhwa, "manhwa"),
        (KitsuMangaSubtype::Manhua, "manhua"),
        (KitsuMangaSubtype::Oneshot, "oneshot"),
        (KitsuMangaSubtype::Doujin, "doujin"),
        (KitsuMangaSubtype::Oel, "oel"),
        (KitsuMangaSubtype::Novel, "novel"),
        (KitsuMangaSubtype::Unknown, "unknown"),
    ];

    fn manga_data() -> SearchCandidateData {
        let mut data = candidate_data();
        data.provider = KITSU_PROVIDER_ID.into();
        data.kind = "manga".into();
        data.image_url = None;
        data
    }

    #[test]
    fn kitsu_anime_identity_is_a_release_without_manga_classification() {
        let mut data = manga_data();
        data.kind = "anime".into();
        data.image_url = Some("https://media.kitsu.app/anime/42/poster_image/small.jpeg".into());
        let candidate = SearchCandidate::try_new(data.clone()).unwrap();
        assert_eq!(candidate.identifier().namespace(), "kitsu.anime");
        assert_eq!(candidate.identifier().grain(), Grain::Release);
        assert_eq!(candidate.identifier().value(), "42");
        assert_ne!(
            candidate.identifier(),
            SearchCandidate::try_new(manga_data()).unwrap().identifier()
        );
        assert!(!candidate.to_json().unwrap().contains("kitsu_manga_subtype"));
        data.kitsu_manga_subtype = Some(KitsuMangaSubtype::Manga);
        assert!(SearchCandidate::try_new(data).is_err());
    }

    #[test]
    fn kitsu_absence_preserves_preexisting_canonical_candidate_bytes() {
        for json in [
            r#"{"provider":"tmdb","provider_id":"42","kind":"movie","title":"A film","original_title":null,"release_year":2026,"authors":[],"image_url":"https://image.tmdb.org/t/p/w500/film.jpg","overview":"A description."}"#,
            r#"{"provider":"google-books","provider_id":"42","kind":"book","title":"A film","original_title":null,"release_year":2026,"authors":[],"image_url":null,"overview":"A description.","google_books_print_type":"BOOK"}"#,
        ] {
            let candidate = SearchCandidate::from_json(json).unwrap();
            assert_eq!(candidate.data().kitsu_manga_subtype, None);
            assert_eq!(candidate.to_json().unwrap().as_bytes(), json.as_bytes());
        }
        let candidate = SearchCandidate::try_new(manga_data()).unwrap();
        assert_eq!(candidate.identifier().namespace(), "kitsu.manga");
        assert_eq!(candidate.identifier().grain(), Grain::Work);
        assert_eq!(candidate.identifier().value(), "42");
        assert!(!candidate.to_json().unwrap().contains("kitsu_manga_subtype"));
    }

    #[test]
    fn kitsu_subtypes_round_trip_without_turning_novel_or_unknown_into_absence() {
        for (value, wire) in VALUES {
            assert_eq!(value.as_str(), wire);
            assert_eq!(KitsuMangaSubtype::parse_claim(wire), Some(value));
            assert_eq!(KitsuMangaSubtype::from_source(Some(wire)), value);
            assert_eq!(
                serde_json::to_string(&value).unwrap(),
                format!("\"{wire}\"")
            );
            let mut data = manga_data();
            data.kitsu_manga_subtype = Some(value);
            let candidate = SearchCandidate::try_new(data).unwrap();
            let json = candidate.to_json().unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&json).unwrap()["kitsu_manga_subtype"],
                wire
            );
            assert_eq!(SearchCandidate::from_json(&json).unwrap(), candidate);
        }
        assert_eq!(
            KitsuMangaSubtype::from_source(None),
            KitsuMangaSubtype::Unknown
        );
        for invalid in [
            "",
            "MANGA",
            " manga",
            "manga ",
            "one_shot",
            "doujinshi",
            "NEW_TYPE",
        ] {
            assert_eq!(KitsuMangaSubtype::parse_claim(invalid), None);
            assert_eq!(
                KitsuMangaSubtype::from_source(Some(invalid)),
                KitsuMangaSubtype::Unknown
            );
        }
        for invalid in [
            serde_json::json!("one_shot"),
            serde_json::json!(42),
            serde_json::json!(["manga"]),
        ] {
            let mut wire = serde_json::to_value(manga_data()).unwrap();
            wire["kitsu_manga_subtype"] = invalid;
            assert!(SearchCandidate::from_json(&wire.to_string()).is_err());
        }
    }

    #[test]
    fn kitsu_fact_rejects_wrong_provider_kind_cross_provider_facts_and_invalid_ids() {
        for (provider, kind) in [
            ("tmdb", "movie"),
            ("google-books", "book"),
            ("kitsu", "anime"),
        ] {
            let mut data = manga_data();
            data.provider = provider.into();
            data.kind = kind.into();
            data.kitsu_manga_subtype = Some(KitsuMangaSubtype::Manga);
            let json = serde_json::to_string(&data).unwrap();
            assert_eq!(
                SearchCandidate::try_new(data).unwrap_err(),
                SearchEvidenceError::InvalidCandidate
            );
            assert!(SearchCandidate::from_json(&json).is_err());
        }
        for value in [
            GoogleBooksPrintType::Book,
            GoogleBooksPrintType::Magazine,
            GoogleBooksPrintType::Unknown,
        ] {
            let mut data = manga_data();
            data.kitsu_manga_subtype = Some(KitsuMangaSubtype::Manga);
            data.google_books_print_type = Some(value);
            assert!(SearchCandidate::try_new(data).is_err());
        }
        for id in [
            "",
            "0",
            "-1",
            "+1",
            "01",
            " 42",
            "42 ",
            "1.0",
            "1e2",
            "４２",
            "42/chapters",
        ] {
            let mut data = manga_data();
            data.provider_id = id.into();
            assert!(SearchCandidate::try_new(data).is_err(), "accepted {id:?}");
        }
    }

    #[test]
    fn kitsu_metadata_preserves_all_observations_and_the_supplied_response_policy() {
        let at = lifetime().created_at() + Duration::nanoseconds(123);
        let digest = Sha256Digest::from_bytes(&[17; 32]);
        let locale = MetadataLocale::try_new("ja-JP").unwrap();
        let region = MetadataRegion::try_new("JP").unwrap();
        for (value, wire) in VALUES {
            for (status, expires_at) in [
                (FieldClaimStatus::Fresh, Some(at + Duration::seconds(120))),
                (FieldClaimStatus::Stale, None),
            ] {
                let mut data = manga_data();
                data.kitsu_manga_subtype = Some(value);
                let candidate = SearchCandidate::try_new(data).unwrap();
                let fields = provider_candidate_metadata_fields(
                    &candidate,
                    Some(locale.clone()),
                    Some(region.clone()),
                    &digest,
                    ReceivedAt::from_application_clock(at),
                    expires_at,
                    status,
                )
                .unwrap();
                let native: Vec<_> = fields
                    .iter()
                    .filter(|field| field.field_key().as_str() == KITSU_MANGA_SUBTYPE_FIELD_KEY)
                    .collect();
                assert_eq!(native.len(), 1);
                let claim = native[0].claim();
                let title = fields
                    .iter()
                    .find(|field| field.field_key().as_str() == TITLE_FIELD_KEY)
                    .unwrap();
                assert!(valid_provider_native_fact(native[0].field_key(), claim));
                assert_eq!(claim.value(), wire);
                assert_eq!(claim.fetched_at(), at);
                assert_eq!(claim.expires_at(), expires_at);
                assert_eq!(claim.initial_status(), status);
                assert_eq!(claim.provenance(), title.claim().provenance());
                let provenance = claim.provenance();
                assert!(provenance.is_complete());
                assert_eq!(
                    provenance.provider_id().unwrap().as_str(),
                    KITSU_PROVIDER_ID
                );
                assert_eq!(provenance.source_namespace().as_str(), "kitsu.manga");
                assert_eq!(provenance.source_identifier(), Some("42"));
                assert_eq!(provenance.locale(), Some(&locale));
                assert_eq!(provenance.region(), Some(&region));
                assert_eq!(provenance.evidence_digest(), Some(&digest));
                for reuse in [
                    ProviderResponseReuse::NoStore,
                    ProviderResponseReuse::ValidateEveryReuse,
                    ProviderResponseReuse::ValidateWhenStale,
                    ProviderResponseReuse::Reusable,
                ] {
                    let policy = ProviderResponseCachePolicy::new(
                        reuse,
                        at,
                        std::time::Duration::ZERO,
                        None,
                        None,
                    );
                    let command = CreateProviderRecordCommand::new(
                        RequestCorrelationId::new_v7(),
                        RequestAccessContext::new(
                            WorkspaceId::new_v7(),
                            ProfileId::new_v7(),
                            ClientId::new_v7(),
                            CredentialId::new_v7(),
                            ProfileGrantId::new_v7(),
                            1,
                        ),
                        Grain::Work,
                        candidate.identifier().clone(),
                        fields.clone(),
                        policy,
                    );
                    assert_eq!(command.response_policy(), &policy);
                    assert_eq!(command.fields(), fields.as_slice());
                }
            }
        }
    }

    #[test]
    fn kitsu_unknown_is_an_observation_but_absence_does_not_invent_a_claim() {
        let at = lifetime().created_at();
        let project = |data| {
            provider_candidate_metadata_fields(
                &SearchCandidate::try_new(data).unwrap(),
                None,
                None,
                &Sha256Digest::from_bytes(&[18; 32]),
                ReceivedAt::from_application_clock(at),
                Some(at + Duration::seconds(120)),
                FieldClaimStatus::Fresh,
            )
            .unwrap()
        };
        let absent = project(manga_data());
        assert!(absent
            .iter()
            .all(|field| field.field_key().as_str() != KITSU_MANGA_SUBTYPE_FIELD_KEY));
        let mut data = manga_data();
        data.kitsu_manga_subtype = Some(KitsuMangaSubtype::from_source(None));
        let observed = project(data);
        assert_eq!(observed.len(), absent.len() + 1);
        assert_eq!(
            observed
                .iter()
                .find(|field| field.field_key().as_str() == KITSU_MANGA_SUBTYPE_FIELD_KEY)
                .unwrap()
                .claim()
                .value(),
            "unknown"
        );
    }

    #[test]
    fn kitsu_reserved_fact_validates_provider_namespace_and_typed_value_independently() {
        let key = FieldKey::try_new(KITSU_MANGA_SUBTYPE_FIELD_KEY).unwrap();
        let at = lifetime().created_at();
        for (provider, namespace, value, valid) in [
            ("kitsu", "kitsu.manga", "manga", true),
            ("kitsu", "kitsu.manga", "novel", true),
            ("kitsu", "kitsu.manga", "unknown", true),
            ("tmdb", "kitsu.manga", "manga", false),
            ("google-books", "kitsu.manga", "manga", false),
            ("kitsu", "kitsu.anime", "manga", false),
            ("kitsu", "googlebooks.volume", "manga", false),
            ("kitsu", "kitsu.manga", "BOOK", false),
            ("kitsu", "kitsu.manga", "one_shot", false),
        ] {
            let provenance = FieldClaimProvenance::try_new(
                MetadataProviderId::try_new(provider).unwrap(),
                NamespaceKey::try_new(namespace).unwrap(),
                "42",
                None,
                None,
                None,
                Sha256Digest::from_bytes(&[19; 32]),
            )
            .unwrap();
            let claim = FieldClaim::try_new_unbound_provider(
                MetadataClaimId::new_v7(),
                value,
                provenance,
                ReceivedAt::from_application_clock(at),
                Some(at + Duration::seconds(120)),
                FieldClaimStatus::Fresh,
            )
            .unwrap();
            assert_eq!(
                valid_provider_native_fact(&key, &claim),
                valid,
                "{provider}/{namespace}/{value}"
            );
        }
    }
}
