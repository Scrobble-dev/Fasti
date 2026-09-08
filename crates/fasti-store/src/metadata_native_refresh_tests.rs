mod metadata_native_refresh_tests {
    use super::*;
    use fasti_application::{ReadMetadataRefreshReceiptCommand, GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY};

    fn fixture() -> (TestNode, PreparedMetadataRefresh, ProviderCapabilityState) {
        let node = TestNode::new();
        let mapping = provider_identity_mapping(GOOGLE_BOOKS_PROVIDER_ID, "book").unwrap();
        register_mapping(&node, mapping);
        let at = received(now().timestamp() - 30);
        let record = node
            .kernel
            .create_provider_record(CreateProviderRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Edition,
                mapping.identifier("42").unwrap(),
                vec![provider_field(
                    GOOGLE_BOOKS_PROVIDER_ID,
                    mapping.namespace(),
                    "42",
                    TITLE_FIELD_KEY,
                    "Publication",
                    at,
                )],
                fixture_response_policy(at),
            ))
            .unwrap()
            .record_id();
        node.kernel
            .authorize_and_configure_projection(ConfigureMetadataProjectionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                MetadataProjectionPolicy::new(
                    node.access.profile_id(),
                    None,
                    Some(MetadataLocale::try_new("en-US").unwrap()),
                    None,
                    true,
                    LastKnownGoodPolicy::Allow,
                ),
                None,
                vec![MetadataFieldGroup::BasicInfo],
                Vec::new(),
            ))
            .unwrap();
        let provider = MetadataProviderId::try_new(GOOGLE_BOOKS_PROVIDER_ID).unwrap();
        let prepared = node
            .kernel
            .authorize_and_prepare_refresh(PrepareMetadataRefreshCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record,
                provider,
                vec![MetadataFieldGroup::BasicInfo],
            ))
            .unwrap();
        let state = ProviderCapabilityState::try_new(
            ProviderId::try_new(GOOGLE_BOOKS_PROVIDER_ID).unwrap(),
            ProviderCapabilityId::try_new("metadata.read").unwrap(),
            ProviderCapabilityStatus::Available,
            1,
            CredentialRequirement::OptionalApiKey,
            None,
            ProviderCredentialStatus::Optional,
            ConfigurationDigest::parse("a".repeat(64)).unwrap(),
            ProviderCheckMetadata::never_run(),
            ProviderCheckMetadata::never_run(),
        )
        .unwrap();
        node.kernel
            .put_provider_capability_state(node.access.workspace_id(), state.clone())
            .unwrap();
        (node, prepared, state)
    }

    fn command(
        node: &TestNode,
        prepared: &PreparedMetadataRefresh,
        state: &ProviderCapabilityState,
        value: &str,
        locale: &str,
        at: ReceivedAt,
        no_cache: bool,
    ) -> CommitMetadataRefreshCommand {
        let provider = MetadataProviderId::try_new(GOOGLE_BOOKS_PROVIDER_ID).unwrap();
        let field = ProviderMetadataField::new(
            field_key(GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY),
            FieldClaim::try_new_unbound_provider(
                MetadataClaimId::new_v7(),
                value,
                FieldClaimProvenance::try_new(
                    provider.clone(),
                    ns("googlebooks.volume"),
                    "42",
                    Some(MetadataLocale::try_new(locale).unwrap()),
                    None,
                    None,
                    digest("d"),
                )
                .unwrap(),
                at,
                (!no_cache).then_some(at.value() + chrono::Duration::seconds(120)),
                if no_cache {
                    FieldClaimStatus::Stale
                } else {
                    FieldClaimStatus::Fresh
                },
            )
            .unwrap(),
        );
        CommitMetadataRefreshCommand::new(
            RequestCorrelationId::new_v7(),
            node.access,
            fasti_domain::OperationId::new_v7(),
            digest("e"),
            prepared.clone(),
            provider.clone(),
            state.clone(),
            vec![field],
            Vec::new(),
            Vec::new(),
            MetadataAttribution::try_new(
                provider,
                "Google Books metadata",
                "https://developers.google.com/books/",
            )
            .unwrap(),
            ProviderResponseCachePolicy::new(
                if no_cache {
                    ProviderResponseReuse::ValidateEveryReuse
                } else {
                    ProviderResponseReuse::Reusable
                },
                at.value(),
                std::time::Duration::ZERO,
                Some(std::time::Duration::from_secs(120)),
                None,
            ),
        )
    }

    #[test]
    fn refresh_keeps_raw_native_observation_without_projecting_conflicting_locales() {
        let (node, prepared, state) = fixture();
        let earlier = command(
            &node,
            &prepared,
            &state,
            "MAGAZINE",
            "fr-FR",
            received(now().timestamp() - 20),
            false,
        );
        let earlier_id = earlier.fields()[0].claim().claim_id();
        node.kernel.authorize_and_commit_refresh(earlier).unwrap();
        let current = command(
            &node,
            &prepared,
            &state,
            "BOOK",
            "en-US",
            received(now().timestamp() - 10),
            false,
        );
        let current_claim = current.fields()[0].claim().clone();
        let live = node.kernel.authorize_and_commit_refresh(current).unwrap();
        assert_eq!(live.field_claims().len(), 1);
        let raw = live.field_claims()[0].claim();
        assert_eq!(raw.claim_id(), current_claim.claim_id());
        assert_eq!(raw.value(), "BOOK");
        assert_eq!(raw.provenance(), current_claim.provenance());
        assert_eq!(raw.fetched_at(), current_claim.fetched_at());
        assert_eq!(live.projections().len(), 1);
        let projection = live.projections()[0].resolved_field();
        assert_eq!(projection.tier(), FieldResolutionTier::Empty);
        assert!(projection.value().is_none());
        assert!(projection.provenance().is_none());
        let connection = node.kernel.inner.connection.lock().unwrap();
        let retained: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM metadata_claims WHERE claim_id IN (?1, ?2)",
                params![earlier_id.to_string(), current_claim.claim_id().to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(retained, 2);
        assert!(load_field_claims(
            &connection,
            node.access.workspace_id(),
            prepared.record_id(),
            &field_key(GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY),
            CapabilityKey::RefreshMetadataClaims,
            RequestCorrelationId::new_v7(),
            now()
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn isolated_no_cache_native_refresh_projects_once_without_cached_reuse() {
        let (node, prepared, state) = fixture();
        let current = command(
            &node,
            &prepared,
            &state,
            "BOOK",
            "en-US",
            received(now().timestamp() - 10),
            true,
        );
        let claim_id = current.fields()[0].claim().claim_id();
        let operation = current.operation_id();
        let live = node
            .kernel
            .authorize_and_commit_refresh(current.clone())
            .unwrap();
        assert_eq!(live.field_claims().len(), 1);
        assert_eq!(live.field_claims()[0].claim().claim_id(), claim_id);
        assert_eq!(live.field_claims()[0].claim().value(), "BOOK");
        assert_eq!(live.projections().len(), 1);
        let projected = live.projections()[0].resolved_field();
        assert_eq!(projected.value(), Some("BOOK"));
        assert_eq!(projected.tier(), FieldResolutionTier::LastKnownGood);
        assert_eq!(projected.provenance().unwrap().claim_id(), claim_id);
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            assert!(load_field_claims(
                &connection,
                node.access.workspace_id(),
                prepared.record_id(),
                &field_key(GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY),
                CapabilityKey::RefreshMetadataClaims,
                RequestCorrelationId::new_v7(),
                now()
            )
            .unwrap()
            .is_empty());
            let stored: String = connection
                .query_row(
                    "SELECT response_policy_json FROM metadata_claims WHERE claim_id=?1",
                    [claim_id.to_string()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                ProviderResponseCachePolicy::from_canonical_json(&stored)
                    .unwrap()
                    .reuse(),
                ProviderResponseReuse::ValidateEveryReuse
            );
        }
        let read = ReadMetadataRefreshReceiptCommand::new(
            RequestCorrelationId::new_v7(),
            node.access,
            operation,
            digest("e"),
            prepared.record_id(),
            MetadataProviderId::try_new(GOOGLE_BOOKS_PROVIDER_ID).unwrap(),
        );
        assert_eq!(
            node.kernel
                .authorize_and_read_refresh_receipt(read)
                .unwrap_err()
                .code(),
            ProblemCode::MetadataClaimStale
        );
        assert_eq!(
            node.kernel
                .authorize_and_commit_refresh(current)
                .unwrap_err()
                .code(),
            ProblemCode::MetadataClaimStale
        );
    }
}
