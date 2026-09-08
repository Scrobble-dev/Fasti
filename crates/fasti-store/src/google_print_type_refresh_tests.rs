mod google_print_type_refresh_tests {
    use super::*;
    use fasti_application::GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY;

    #[test]
    fn refresh_native_fact_admission_checks_value_and_provider_coordinates() {
        let node = TestNode::new();
        let at = received(now().timestamp() - 1);
        for (provider, kind) in [
            (GOOGLE_BOOKS_PROVIDER_ID, "book"),
            (TMDB_PROVIDER_ID, "movie"),
        ] {
            let mapping = provider_identity_mapping(provider, kind).unwrap();
            let provider_id = MetadataProviderId::try_new(provider).unwrap();
            let prepared = PreparedMetadataRefresh::new(
                RecordId::new_v7(),
                mapping.grain(),
                mapping.identifier("42").unwrap(),
                vec![MetadataFieldGroup::BasicInfo],
                digest("b"),
            );
            let state = ProviderCapabilityState::try_new(
                ProviderId::try_new(provider).unwrap(),
                ProviderCapabilityId::try_new("metadata.read").unwrap(),
                ProviderCapabilityStatus::Available,
                1,
                CredentialRequirement::ApiKey,
                Some(CredentialReference::try_new("test-provider-key").unwrap()),
                ProviderCredentialStatus::StoredUnverified,
                ConfigurationDigest::parse("a".repeat(64)).unwrap(),
                ProviderCheckMetadata::never_run(),
                ProviderCheckMetadata::never_run(),
            )
            .unwrap();
            for value in ["BOOK", "MAGAZINE", "UNKNOWN", "book", "NOVEL"] {
                let field = provider_field(
                    provider,
                    mapping.namespace(),
                    "42",
                    GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY,
                    value,
                    at,
                );
                let command = CommitMetadataRefreshCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    fasti_domain::OperationId::new_v7(),
                    digest("c"),
                    prepared.clone(),
                    provider_id.clone(),
                    state.clone(),
                    vec![field],
                    Vec::new(),
                    Vec::new(),
                    MetadataAttribution::try_new(
                        provider_id.clone(),
                        "Provider metadata",
                        "https://example.com/metadata",
                    )
                    .unwrap(),
                    fixture_response_policy(at),
                );
                let result = validate_refresh_commit(
                    &command,
                    &prepared,
                    CapabilityKey::RefreshMetadataClaims,
                    RequestCorrelationId::new_v7(),
                );
                let allowed = provider == GOOGLE_BOOKS_PROVIDER_ID
                    && matches!(value, "BOOK" | "MAGAZINE" | "UNKNOWN");
                assert_eq!(result.is_ok(), allowed, "{provider}: {value}");
                if let Err(problem) = result {
                    assert_eq!(problem.code(), ProblemCode::IntegrityFailed);
                }
            }
        }
    }

    #[test]
    fn projection_only_refresh_receipts_validate_native_fact_evidence() {
        for (provider, source, value, allowed) in [
            (GOOGLE_BOOKS_PROVIDER_ID, "googlebooks.volume", "BOOK", true),
            (
                GOOGLE_BOOKS_PROVIDER_ID,
                "googlebooks.volume",
                "MAGAZINE",
                true,
            ),
            (
                GOOGLE_BOOKS_PROVIDER_ID,
                "googlebooks.volume",
                "UNKNOWN",
                true,
            ),
            (
                GOOGLE_BOOKS_PROVIDER_ID,
                "googlebooks.volume",
                "book",
                false,
            ),
            (
                GOOGLE_BOOKS_PROVIDER_ID,
                "googlebooks.volume",
                "NOVEL",
                false,
            ),
            (TMDB_PROVIDER_ID, "tmdb.movie", "BOOK", false),
            (GOOGLE_BOOKS_PROVIDER_ID, "tmdb.movie", "BOOK", false),
        ] {
            let node = TestNode::new();
            let record = RecordId::new_v7();
            let at = received(now().timestamp() - 1);
            let field = provider_field(
                provider,
                source,
                "42",
                GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY,
                value,
                at,
            );
            let claim = FieldClaim::try_new_provider(
                field.claim().claim_id(),
                record,
                field.field_key().clone(),
                value,
                field.claim().provenance().clone(),
                at,
                field.claim().expires_at(),
                FieldClaimStatus::Fresh,
            )
            .unwrap();
            let resolved = ResolvedField::try_from_snapshot(
                FieldResolutionTier::FallbackProviderClaim,
                Some(value.to_owned()),
                Some(ns(source)),
                false,
                Some((&claim, FieldClaimStatus::Fresh)),
            )
            .unwrap();
            let projection = MetadataProjection::try_new(
                node.access.profile_id(),
                record,
                field.field_key().clone(),
                resolved,
                at,
            )
            .unwrap();
            // No top-level claim can trigger receipt_field_claim: the native
            // evidence exists only inside this provider-backed projection.
            let outcome = RefreshMetadataClaimsOutcome::new(
                Vec::new(),
                Vec::new(),
                vec![projection],
                Vec::new(),
                Vec::new(),
            );
            let provider_id = MetadataProviderId::try_new(provider).unwrap();
            let capability = CapabilityKey::RefreshMetadataClaims;
            let correlation = RequestCorrelationId::new_v7();
            let encoded = encode_refresh_receipt_outcome(
                record,
                &provider_id,
                &outcome,
                capability,
                correlation,
            )
            .unwrap();
            let response: RefreshMetadataClaimsResponse = serde_json::from_str(&encoded).unwrap();
            assert!(response.claims.is_empty());
            assert_eq!(response.projections.len(), 1);
            assert!(response.projections[0].provenance.is_some());
            let decoded = decode_refresh_receipt_outcome(
                &encoded,
                record,
                &provider_id,
                node.access.profile_id(),
                capability,
                correlation,
            );
            assert_eq!(
                decoded.is_ok(),
                allowed,
                "projection {provider}/{source}: {value}"
            );
            match decoded {
                Ok(decoded) => assert_eq!(decoded, outcome),
                Err(problem) => assert_eq!(problem.code(), ProblemCode::IntegrityFailed),
            }
        }
    }

    #[test]
    fn native_field_override_remains_distinct_from_provider_evidence() {
        let node = TestNode::new();
        let record = RecordId::new_v7();
        let provider = MetadataProviderId::try_new(GOOGLE_BOOKS_PROVIDER_ID).unwrap();
        let capability = CapabilityKey::RefreshMetadataClaims;
        let correlation = RequestCorrelationId::new_v7();
        for value in ["BOOK", "Not a provider enum"] {
            let resolved = ResolvedField::try_from_snapshot(
                FieldResolutionTier::UserOverride,
                Some(value.to_owned()),
                None,
                false,
                None,
            )
            .unwrap();
            let projection = MetadataProjection::try_new(
                node.access.profile_id(),
                record,
                field_key(GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY),
                resolved,
                received(now().timestamp() - 1),
            )
            .unwrap();
            let outcome = RefreshMetadataClaimsOutcome::new(
                Vec::new(),
                Vec::new(),
                vec![projection],
                Vec::new(),
                Vec::new(),
            );
            let encoded = encode_refresh_receipt_outcome(
                record,
                &provider,
                &outcome,
                capability,
                correlation,
            )
            .unwrap();
            let decoded = decode_refresh_receipt_outcome(
                &encoded,
                record,
                &provider,
                node.access.profile_id(),
                capability,
                correlation,
            )
            .unwrap();
            assert_eq!(decoded, outcome);
            let resolved = decoded.projections()[0].resolved_field();
            assert_eq!(resolved.tier(), FieldResolutionTier::UserOverride);
            assert_eq!(resolved.value(), Some(value));
            assert!(resolved.provenance().is_none());
            assert!(resolved.source().is_none());

            // A user value, even BOOK, is not provider evidence. Merely changing
            // its tier and source must not manufacture the missing provenance.
            let mut response: RefreshMetadataClaimsResponse =
                serde_json::from_str(&encoded).unwrap();
            assert!(response.claims.is_empty());
            assert!(response.projections[0].provenance.is_none());
            response.projections[0].tier = MetadataProjectionTierDto::FallbackProviderClaim;
            response.projections[0].source_namespace = Some("googlebooks.volume".to_owned());
            let mislabeled = serde_json::to_string(&response).unwrap();
            let error = decode_refresh_receipt_outcome(
                &mislabeled,
                record,
                &provider,
                node.access.profile_id(),
                capability,
                correlation,
            )
            .unwrap_err();
            assert_eq!(error.code(), ProblemCode::IntegrityFailed);
        }
    }

    #[test]
    fn stored_native_facts_and_refresh_receipts_reject_untrusted_values() {
        for (provider, kind, value, allowed) in [
            (GOOGLE_BOOKS_PROVIDER_ID, "book", "BOOK", true),
            (GOOGLE_BOOKS_PROVIDER_ID, "book", "MAGAZINE", true),
            (GOOGLE_BOOKS_PROVIDER_ID, "book", "UNKNOWN", true),
            (GOOGLE_BOOKS_PROVIDER_ID, "book", "book", false),
            (GOOGLE_BOOKS_PROVIDER_ID, "book", "NOVEL", false),
            (TMDB_PROVIDER_ID, "movie", "BOOK", false),
        ] {
            let node = TestNode::new();
            let mapping = provider_identity_mapping(provider, kind).unwrap();
            register_mapping(&node, mapping);
            let record = node
                .kernel
                .create_record(CreateRecordCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    mapping.grain(),
                ))
                .unwrap()
                .record_id();
            let at = received(now().timestamp() - 1);
            let field = provider_field(
                provider,
                mapping.namespace(),
                "42",
                GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY,
                value,
                at,
            );
            let capability = CapabilityKey::RefreshMetadataClaims;
            let correlation = RequestCorrelationId::new_v7();
            let policy = fixture_response_policy(at).to_canonical_json();
            let connection = node.kernel.inner.connection.lock().unwrap();
            // Bypass command admission to model generic restored claim rows.
            // Loading them must still enforce the reserved native-fact rule.
            write_field_claim(
                &connection,
                node.access.workspace_id(),
                record,
                field.field_key(),
                field.claim(),
                capability,
                correlation,
                Some(&policy),
            )
            .unwrap();
            let loaded = load_field_claims(
                &connection,
                node.access.workspace_id(),
                record,
                field.field_key(),
                capability,
                correlation,
                at.value(),
            );
            assert_eq!(loaded.is_ok(), allowed, "stored {provider}: {value}");
            match loaded {
                Ok(claims) => {
                    assert_eq!(claims.len(), 1);
                    assert_eq!(claims[0].value(), value);
                }
                Err(problem) => assert_eq!(problem.code(), ProblemCode::IntegrityFailed),
            }

            let claim = FieldClaim::try_new_provider(
                field.claim().claim_id(),
                record,
                field.field_key().clone(),
                value,
                field.claim().provenance().clone(),
                at,
                field.claim().expires_at(),
                field.claim().initial_status(),
            )
            .unwrap();
            let outcome = RefreshMetadataClaimsOutcome::new(
                vec![FieldClaimView::new(claim, FieldClaimStatus::Fresh)],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
            let provider_id = MetadataProviderId::try_new(provider).unwrap();
            let encoded = encode_refresh_receipt_outcome(
                record,
                &provider_id,
                &outcome,
                capability,
                correlation,
            )
            .unwrap();
            let decoded = decode_refresh_receipt_outcome(
                &encoded,
                record,
                &provider_id,
                node.access.profile_id(),
                capability,
                correlation,
            );
            assert_eq!(decoded.is_ok(), allowed, "receipt {provider}: {value}");
            match decoded {
                Ok(decoded) => assert_eq!(decoded, outcome),
                Err(problem) => assert_eq!(problem.code(), ProblemCode::IntegrityFailed),
            }
        }
    }
}
