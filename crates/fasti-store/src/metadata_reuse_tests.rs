mod metadata_reuse_tests {
    use super::*;
    use fasti_application::{
        LocalSearchRequest, ProviderResponseCachePolicy, ProviderResponseReuse,
        SearchPersistencePort,
    };
    use fasti_domain::{MetadataRegion, SearchQuery};
    use rusqlite::types::Value;
    use std::time::Duration;

    fn provenance_for(
        provider: &str,
        namespace: &str,
        source: &str,
        locale: &str,
        region: Option<&str>,
    ) -> FieldClaimProvenance {
        FieldClaimProvenance::try_new(
            MetadataProviderId::try_new(provider).unwrap(),
            ns(namespace),
            source,
            Some(MetadataLocale::try_new(locale).unwrap()),
            region.map(|value| MetadataRegion::try_new(value).unwrap()),
            Some("v3".into()),
            digest("a"),
        )
        .unwrap()
    }

    fn seed_response(
        node: &TestNode,
        record: RecordId,
        title: &str,
        provenance: FieldClaimProvenance,
        fetched: chrono::DateTime<chrono::Utc>,
        reuse: ProviderResponseReuse,
    ) -> (MetadataClaimId, MetadataClaimId) {
        let expiry = (reuse != ProviderResponseReuse::ValidateEveryReuse)
            .then_some(fetched + chrono::Duration::seconds(120));
        let status = if expiry.is_some() {
            FieldClaimStatus::Fresh
        } else {
            FieldClaimStatus::Stale
        };
        let policy = ProviderResponseCachePolicy::new(
            reuse,
            fetched,
            Duration::ZERO,
            Some(Duration::from_secs(120)),
            None,
        )
        .to_canonical_json();
        let field = FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            record,
            field_key(TITLE_FIELD_KEY),
            title,
            provenance.clone(),
            ReceivedAt::from_application_clock(fetched),
            expiry,
            status,
        )
        .unwrap();
        let rating = RatingClaim::try_new(
            MetadataClaimId::new_v7(),
            record,
            8_000,
            RatingScale::try_new(0, 10_000).unwrap(),
            provenance,
            ReceivedAt::from_application_clock(fetched),
            expiry,
            status,
        )
        .unwrap();
        let connection = node.kernel.inner.connection.lock().unwrap();
        write_field_claim(
            &connection,
            node.access.workspace_id(),
            record,
            &field_key(TITLE_FIELD_KEY),
            &field,
            CapabilityKey::AttachIdentifier,
            RequestCorrelationId::new_v7(),
            Some(&policy),
        )
        .unwrap();
        write_rating_claim(
            &connection,
            node.access.workspace_id(),
            &rating,
            CapabilityKey::AttachIdentifier,
            RequestCorrelationId::new_v7(),
            Some(&policy),
        )
        .unwrap();
        (field.claim_id(), rating.claim_id())
    }

    fn rows(node: &TestNode) -> Vec<Vec<Vec<Value>>> {
        let connection = node.kernel.inner.connection.lock().unwrap();
        [
            "records",
            "metadata_field_claims",
            "metadata_claim_provenance",
            "metadata_claims",
            "metadata_rating_claims",
            "metadata_profile_field_overrides",
            "local_search_grams",
            "workspace_revisions",
        ]
        .into_iter()
        .map(|table| {
            let sql = format!("SELECT * FROM {table}");
            let columns = connection.prepare(&sql).unwrap().column_count();
            let order = (1..=columns)
                .map(|index| index.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let mut statement = connection
                .prepare(&format!("{sql} ORDER BY {order}"))
                .unwrap();
            statement
                .query_map([], |row| {
                    (0..columns)
                        .map(|index| row.get::<_, Value>(index))
                        .collect()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        })
        .collect()
    }

    fn projection(node: &TestNode, record: RecordId) -> MetadataProjectionView {
        node.kernel
            .authorize_and_read_projection(ReadMetadataProjectionQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record,
                true,
            ))
            .unwrap()
    }

    fn assert_public_title(node: &TestNode, record: RecordId, expected: Option<&str>) {
        let view = projection(node, record);
        let title = view
            .fields()
            .iter()
            .find(|field| field.field_key().as_str() == TITLE_FIELD_KEY)
            .and_then(|field| field.resolved_field().value());
        assert_eq!(title, expected, "public metadata projection");
        let records = node
            .kernel
            .list_records(ListRecordsQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
            ))
            .unwrap()
            .into_records();
        let summary = records
            .iter()
            .find(|value| value.record_id() == record)
            .unwrap();
        assert_eq!(
            summary.title().value(),
            expected,
            "Record-list batch resolver"
        );
    }

    fn local_ids(node: &TestNode, text: &str) -> Vec<RecordId> {
        node.kernel
            .search_local_records(&LocalSearchRequest {
                correlation_id: RequestCorrelationId::new_v7(),
                access: node.access.into(),
                query: SearchQuery::try_new(text).unwrap(),
                grains: Vec::new(),
                after: None,
            })
            .unwrap()
            .records
            .into_iter()
            .map(|record| record.record_id())
            .collect()
    }

    #[test]
    fn newer_restricted_response_hides_old_payload_from_projection_list_search_and_ratings() {
        let node = TestNode::new();
        let (record, _, _) = refresh_fixture(&node);
        let observed = now() - chrono::Duration::seconds(60);
        let provenance = provenance_for("tmdb", "tmdb.movie", "438631", "en-US", None);
        seed_response(
            &node,
            record,
            "Hidden old lighthouse",
            provenance.clone(),
            observed,
            ProviderResponseReuse::Reusable,
        );
        assert_public_title(&node, record, Some("Hidden old lighthouse"));
        assert_eq!(local_ids(&node, "lighthouse"), vec![record]);
        assert_eq!(projection(&node, record).ratings().len(), 1);
        seed_response(
            &node,
            record,
            "Restricted new harbor",
            provenance,
            observed + chrono::Duration::seconds(30),
            ProviderResponseReuse::ValidateEveryReuse,
        );
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            let postings: i64 = connection.query_row(
                "SELECT COUNT(*) FROM local_search_grams WHERE record_id=?1 AND profile_partition='' AND gram IN ('lig','har')",
                [record.to_string()], |row| row.get(0)).unwrap();
            assert_eq!(
                postings, 2,
                "both stored titles still have candidate postings"
            );
        }
        let before = rows(&node);
        assert_public_title(&node, record, None);
        assert!(local_ids(&node, "lighthouse").is_empty());
        assert!(local_ids(&node, "harbor").is_empty());
        assert!(projection(&node, record).ratings().is_empty());
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            assert!(load_rating_claims(
                &connection,
                node.access.workspace_id(),
                record,
                CapabilityKey::ReadMetadataProjection,
                RequestCorrelationId::new_v7(),
                now()
            )
            .unwrap()
            .is_empty());
        }
        assert_eq!(
            rows(&node),
            before,
            "eligibility filtering never deletes evidence or postings"
        );

        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            write_profile_field_override(
                &connection,
                node.access.workspace_id(),
                &ProfileFieldOverride::try_new(
                    node.access.profile_id(),
                    record,
                    field_key(TITLE_FIELD_KEY),
                    "User chosen observatory",
                    ReceivedAt::from_application_clock(now()),
                )
                .unwrap(),
                CapabilityKey::AttachIdentifier,
                RequestCorrelationId::new_v7(),
            )
            .unwrap();
        }
        let overridden = rows(&node);
        assert_public_title(&node, record, Some("User chosen observatory"));
        assert_eq!(local_ids(&node, "observatory"), vec![record]);
        assert!(local_ids(&node, "lighthouse").is_empty());
        assert!(local_ids(&node, "harbor").is_empty());
        assert!(projection(&node, record).ratings().is_empty());
        assert_eq!(rows(&node), overridden);
    }

    #[test]
    fn independent_response_variants_remain_visible_when_another_variant_is_restricted() {
        for independent in ["provider", "namespace", "source", "locale", "region"] {
            let node = TestNode::new();
            let (record, _, _) = refresh_fixture(&node);
            let observed = now() - chrono::Duration::seconds(60);
            let original = provenance_for("tmdb", "tmdb.movie", "438631", "en-US", Some("IE"));
            seed_response(
                &node,
                record,
                "Blocked old lighthouse",
                original.clone(),
                observed,
                ProviderResponseReuse::Reusable,
            );
            seed_response(
                &node,
                record,
                "Blocked new harbor",
                original,
                observed + chrono::Duration::seconds(30),
                ProviderResponseReuse::ValidateEveryReuse,
            );
            let locale = if independent == "locale" {
                "fr-FR"
            } else {
                "en-US"
            };
            let provenance = provenance_for(
                if independent == "provider" {
                    "other"
                } else {
                    "tmdb"
                },
                if independent == "namespace" {
                    "tmdb.tv"
                } else {
                    "tmdb.movie"
                },
                if independent == "source" {
                    "other-source"
                } else {
                    "438631"
                },
                locale,
                Some(if independent == "region" { "US" } else { "IE" }),
            );
            let (_, rating_id) = seed_response(
                &node,
                record,
                "Independent observatory",
                provenance,
                observed + chrono::Duration::seconds(10),
                ProviderResponseReuse::Reusable,
            );
            node.kernel
                .authorize_and_configure_projection(ConfigureMetadataProjectionCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    MetadataProjectionPolicy::new(
                        node.access.profile_id(),
                        None,
                        Some(MetadataLocale::try_new(locale).unwrap()),
                        None,
                        false,
                        LastKnownGoodPolicy::Allow,
                    ),
                    None,
                    vec![MetadataFieldGroup::BasicInfo],
                    Vec::new(),
                ))
                .unwrap();
            let before = rows(&node);
            assert_public_title(&node, record, Some("Independent observatory"));
            assert_eq!(
                local_ids(&node, "observatory"),
                vec![record],
                "{independent}"
            );
            assert!(local_ids(&node, "lighthouse").is_empty(), "{independent}");
            assert!(local_ids(&node, "harbor").is_empty(), "{independent}");
            let view = projection(&node, record);
            assert_eq!(view.ratings().len(), 1, "{independent}");
            assert_eq!(
                view.ratings()[0].claim().claim_id(),
                rating_id,
                "{independent}"
            );
            assert_eq!(rows(&node), before);
        }
    }

    #[test]
    fn known_policy_beyond_seven_days_is_hidden_without_erasing_historical_rows() {
        let node = TestNode::new();
        let (record, _, _) = refresh_fixture(&node);
        let observed = now() - chrono::Duration::days(8);
        let (field_id, rating_id) = seed_response(
            &node,
            record,
            "Expired lighthouse",
            provenance_for("tmdb", "tmdb.movie", "438631", "en-US", None),
            observed,
            ProviderResponseReuse::Reusable,
        );
        let before = rows(&node);
        assert_public_title(&node, record, None);
        assert!(local_ids(&node, "lighthouse").is_empty());
        assert!(projection(&node, record).ratings().is_empty());
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            let read_at = now();
            assert!(load_field_claims(
                &connection,
                node.access.workspace_id(),
                record,
                &field_key(TITLE_FIELD_KEY),
                CapabilityKey::ReadMetadataProjection,
                RequestCorrelationId::new_v7(),
                read_at
            )
            .unwrap()
            .is_empty());
            assert!(load_rating_claims(
                &connection,
                node.access.workspace_id(),
                record,
                CapabilityKey::ReadMetadataProjection,
                RequestCorrelationId::new_v7(),
                read_at
            )
            .unwrap()
            .is_empty());
            for claim in [field_id, rating_id] {
                let json: Option<String> = connection
                    .query_row(
                        "SELECT response_policy_json FROM metadata_claims WHERE claim_id=?1",
                        [claim.to_string()],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert!(json.is_some(), "historical observation stays durable");
            }
        }
        assert_eq!(rows(&node), before);
    }
}
