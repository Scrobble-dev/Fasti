mod records_artwork {
    use super::*;
    use fasti_application::{
        provider_identity_mapping, AccessAdministrationPort, ConfigureMetadataProjectionCommand,
        MetadataOverrideMutation, MetadataProjectionPort, ProviderMetadataField,
        ProviderResponseCachePolicy, ProviderResponseReuse, ReadMetadataProjectionQuery,
        RevokeCredentialCommand,
    };
    use fasti_domain::{
        FieldClaim, FieldClaimProvenance, FieldClaimStatus, FieldKey, MetadataClaimId,
        MetadataFieldGroup, MetadataProjectionPolicy, MetadataProviderId, NamespaceKey, ProfileId,
        ReceivedAt, Sha256Digest, WorkspaceId,
    };
    use std::cell::Cell;
    use std::time::Duration;

    const POSTER: &str = "https://image.tmdb.org/t/p/w500/original.jpg";

    fn write_poster(
        kernel: &SqliteKernel,
        access: RequestAccessContext,
        record: RecordId,
        url: &str,
        observed: chrono::DateTime<chrono::Utc>,
    ) -> MetadataClaimId {
        let id = MetadataClaimId::new_v7();
        let field = ProviderMetadataField::new(
            FieldKey::try_new("core.poster_url").unwrap(),
            FieldClaim::try_new_unbound_provider(
                id,
                url,
                FieldClaimProvenance::try_new(
                    MetadataProviderId::try_new("tmdb").unwrap(),
                    NamespaceKey::try_new("tmdb.movie").unwrap(),
                    "42",
                    None,
                    None,
                    None,
                    Sha256Digest::from_bytes(&[7; 32]),
                )
                .unwrap(),
                ReceivedAt::from_application_clock(observed),
                Some(observed + chrono::Duration::seconds(120)),
                FieldClaimStatus::Fresh,
            )
            .unwrap(),
        );
        kernel
            .apply_provider_metadata(ApplyProviderMetadataCommand::new(
                RequestCorrelationId::new_v7(),
                access,
                record,
                provider_identity_mapping("tmdb", "movie")
                    .unwrap()
                    .identifier("42")
                    .unwrap(),
                vec![field],
                ProviderResponseCachePolicy::new(
                    ProviderResponseReuse::Reusable,
                    observed,
                    Duration::ZERO,
                    Some(Duration::from_secs(120)),
                    None,
                ),
            ))
            .unwrap();
        id
    }

    #[test]
    fn artwork_selection_requires_current_node_workspace_profile_and_record() {
        let (root, kernel) = new_kernel();
        let store = MemoryStore::default();
        let (access, record) = prepare(&kernel, &store, true);
        let claim = write_poster(
            &kernel,
            access,
            record,
            POSTER,
            chrono::Utc::now() - chrono::Duration::seconds(30),
        );
        let artwork = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
        let locator = artwork.locator("tmdb", POSTER, access, record).unwrap();
        let (selected_access, selected) =
            artwork_selection(&kernel, &store, &artwork, &locator).unwrap();
        assert_eq!(selected_access, access);
        assert_eq!(selected.value(), Some(POSTER));
        assert_eq!(selected.provenance().unwrap().claim_id(), claim);
        assert_eq!(
            selected
                .provenance()
                .unwrap()
                .claim_provenance()
                .provider_id()
                .unwrap()
                .as_str(),
            "tmdb"
        );

        let wrong_workspace = RequestAccessContext::new(
            WorkspaceId::new_v7(),
            access.profile_id(),
            access.client_id(),
            access.credential_id(),
            access.grant_id(),
            access.presented_credential_epoch(),
        );
        let wrong_profile = RequestAccessContext::new(
            access.workspace_id(),
            ProfileId::new_v7(),
            access.client_id(),
            access.credential_id(),
            access.grant_id(),
            access.presented_credential_epoch(),
        );
        for foreign in [wrong_workspace, wrong_profile] {
            let locator = artwork.locator("tmdb", POSTER, foreign, record).unwrap();
            assert!(artwork_selection(&kernel, &store, &artwork, &locator).is_err());
        }
        let (_other_root, other_kernel) = new_kernel();
        let other_store = MemoryStore::default();
        let (_, foreign_record) = prepare(&other_kernel, &other_store, true);
        let other_artwork = ArtworkCache::new(
            root.path().join("artwork"),
            other_kernel.data_root_identity(),
        );
        let wrong_node = other_artwork
            .locator("tmdb", POSTER, access, record)
            .unwrap();
        assert!(artwork_selection(&kernel, &store, &artwork, &wrong_node).is_err());
        let empty_record = kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                access,
                Grain::Film,
            ))
            .unwrap()
            .record_id();
        for wrong_record in [RecordId::new_v7(), foreign_record, empty_record] {
            let locator = artwork
                .locator("tmdb", POSTER, access, wrong_record)
                .unwrap();
            assert!(artwork_selection(&kernel, &store, &artwork, &locator).is_err());
        }
        for malformed in [
            String::new(),
            format!("{locator}.extra"),
            locator.to_ascii_uppercase(),
            "x".repeat(257),
        ] {
            assert!(artwork_selection(&kernel, &store, &artwork, &malformed).is_err());
        }
        assert_eq!(
            artwork_selection(&kernel, &store, &artwork, &locator)
                .unwrap()
                .1,
            selected
        );
        assert!(!artwork.root().exists(), "selection performs no image I/O");
    }

    #[test]
    fn artwork_selection_rechecks_missing_and_revoked_native_credentials() {
        let (root, kernel) = new_kernel();
        let store = MemoryStore::default();
        let (access, record) = prepare(&kernel, &store, true);
        write_poster(&kernel, access, record, POSTER, chrono::Utc::now());
        let artwork = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
        let locator = artwork.locator("tmdb", POSTER, access, record).unwrap();
        assert!(artwork_selection(&kernel, &store, &artwork, &locator).is_ok());
        let missing = MemoryStore::default();
        assert_eq!(
            artwork_selection(&kernel, &missing, &artwork, &locator)
                .unwrap_err()
                .code(),
            "not_authenticated"
        );
        kernel
            .revoke_credential(RevokeCredentialCommand::new(
                RequestCorrelationId::new_v7(),
                access,
                access.credential_id(),
            ))
            .unwrap();
        assert_eq!(
            artwork_selection(&kernel, &store, &artwork, &locator)
                .unwrap_err()
                .code(),
            "not_authenticated"
        );
        assert!(!artwork.root().exists());
    }

    #[test]
    fn artwork_selection_reads_changed_and_removed_provider_selection_again() {
        let (root, kernel) = new_kernel();
        let store = MemoryStore::default();
        let (access, record) = prepare(&kernel, &store, true);
        let observed = chrono::Utc::now() - chrono::Duration::seconds(30);
        write_poster(&kernel, access, record, POSTER, observed);
        let artwork = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
        let locator = artwork.locator("tmdb", POSTER, access, record).unwrap();
        let first = artwork_selection(&kernel, &store, &artwork, &locator).unwrap();
        let newer_url = "https://image.tmdb.org/t/p/w500/changed.jpg";
        let newer_claim = write_poster(
            &kernel,
            access,
            record,
            newer_url,
            observed + chrono::Duration::seconds(10),
        );
        let second = artwork_selection(&kernel, &store, &artwork, &locator).unwrap();
        assert_ne!(first.1, second.1);
        assert_eq!(second.1.value(), Some(newer_url));
        assert_eq!(second.1.provenance().unwrap().claim_id(), newer_claim);
        // A user override removes the provider-owned selection, even though
        // both original immutable provider observations still exist.
        kernel
            .authorize_and_configure_projection(ConfigureMetadataProjectionCommand::new(
                RequestCorrelationId::new_v7(),
                access,
                MetadataProjectionPolicy::default_for_profile(access.profile_id()),
                None,
                vec![MetadataFieldGroup::BasicInfo],
                vec![MetadataOverrideMutation::Set {
                    record_id: record,
                    field_key: FieldKey::try_new("core.poster_url").unwrap(),
                    value: "https://example.com/user-poster.jpg".into(),
                }],
            ))
            .unwrap();
        assert!(artwork_selection(&kernel, &store, &artwork, &locator).is_err());
        let records = kernel
            .list_records(ListRecordsQuery::new(
                RequestCorrelationId::new_v7(),
                access,
            ))
            .unwrap()
            .into_records();
        assert_eq!(
            records[0].poster().value(),
            Some("https://example.com/user-poster.jpg")
        );
        assert!(records[0].poster().provenance().is_none());
        assert!(!artwork.root().exists());
    }

    fn prepare(
        kernel: &SqliteKernel,
        store: &MemoryStore,
        apply: bool,
    ) -> (RequestAccessContext, RecordId) {
        complete_setup(kernel, store).unwrap();
        let access = require_access(kernel, store).unwrap();
        // First-run enrollment grants access but leaves enrichment groups off.
        // Enable the real BasicInfo policy before asserting projection evidence.
        kernel
            .authorize_and_configure_projection(ConfigureMetadataProjectionCommand::new(
                RequestCorrelationId::new_v7(),
                access,
                MetadataProjectionPolicy::default_for_profile(access.profile_id()),
                None,
                vec![MetadataFieldGroup::BasicInfo],
                Vec::new(),
            ))
            .unwrap();
        let mapping = provider_identity_mapping("tmdb", "movie").unwrap();
        kernel
            .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                RequestCorrelationId::new_v7(),
                access,
                mapping.namespace_definition().unwrap(),
            ))
            .unwrap();
        let record = if apply {
            kernel
                .create_record(CreateRecordCommand::new(
                    RequestCorrelationId::new_v7(),
                    access,
                    Grain::Film,
                ))
                .unwrap()
                .record_id()
        } else {
            RecordId::new_v7()
        };
        (access, record)
    }

    fn save(
        kernel: &SqliteKernel,
        access: RequestAccessContext,
        target: RecordId,
        apply: bool,
        failure: &str,
        calls: &Cell<usize>,
    ) -> (Result<RecordId, DesktopProblem>, MetadataClaimId) {
        let observed = chrono::Utc::now();
        let policy = ProviderResponseCachePolicy::new(
            if failure == "no_store" {
                ProviderResponseReuse::NoStore
            } else {
                ProviderResponseReuse::Reusable
            },
            observed,
            Duration::ZERO,
            Some(Duration::from_secs(120)),
            None,
        );
        let claim_id = MetadataClaimId::new_v7();
        let provenance = FieldClaimProvenance::try_new(
            MetadataProviderId::try_new("tmdb").unwrap(),
            NamespaceKey::try_new("tmdb.movie").unwrap(),
            "42",
            None,
            None,
            None,
            Sha256Digest::from_bytes(&[7; 32]),
        )
        .unwrap();
        let make_field = |id, key: &str, fetched: chrono::DateTime<chrono::Utc>| {
            ProviderMetadataField::new(
                FieldKey::try_new(key).unwrap(),
                FieldClaim::try_new_unbound_provider(
                    id,
                    "Original committed title",
                    provenance.clone(),
                    ReceivedAt::from_application_clock(fetched),
                    Some(fetched + chrono::Duration::seconds(120)),
                    FieldClaimStatus::Fresh,
                )
                .unwrap(),
            )
        };
        let mut fields = vec![make_field(claim_id, "core.title", observed)];
        if failure == "invalid_later" {
            fields.push(make_field(
                MetadataClaimId::new_v7(),
                "core.original_title",
                observed + chrono::Duration::seconds(1),
            ));
        }
        let access = if failure == "denied" {
            RequestAccessContext::new(
                access.workspace_id(),
                ProfileId::new_v7(),
                access.client_id(),
                access.credential_id(),
                access.grant_id(),
                access.presented_credential_epoch(),
            )
        } else {
            access
        };
        let identifier = provider_identity_mapping("tmdb", "movie")
            .unwrap()
            .identifier("42")
            .unwrap();
        calls.set(calls.get() + 1);
        let result = if apply {
            kernel
                .apply_provider_metadata(ApplyProviderMetadataCommand::new(
                    RequestCorrelationId::new_v7(),
                    access,
                    target,
                    identifier,
                    fields,
                    policy,
                ))
                .map(|()| target)
        } else {
            kernel
                .create_provider_record(CreateProviderRecordCommand::new(
                    RequestCorrelationId::new_v7(),
                    access,
                    Grain::Film,
                    identifier,
                    fields,
                    policy,
                ))
                .map(|outcome| outcome.record_id())
        }
        .map_err(|problem| DesktopProblem::application(&problem));
        (result, claim_id)
    }

    #[tokio::test]
    async fn denied_or_unadmitted_create_and_apply_never_poll_artwork() {
        for apply in [false, true] {
            for failure in ["denied", "no_store", "invalid_later"] {
                let (root, kernel) = new_kernel();
                let store = MemoryStore::default();
                let (access, target) = prepare(&kernel, &store, apply);
                let artwork =
                    ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
                let before =
                    serde_json::to_value(list_records(&kernel, &store, &artwork, None).unwrap())
                        .unwrap();
                let calls = Cell::new(0);
                let polls = Cell::new(0);
                let (completed, _) = save(&kernel, access, target, apply, failure, &calls);
                let code = completed
                    .as_ref()
                    .expect_err("real Store rejects mutation")
                    .code();
                let result = finish_provider_save(completed, async {
                    polls.set(polls.get() + 1);
                    Err(DesktopProblem::provider("Artwork must not run"))
                })
                .await;
                assert_eq!(result.unwrap_err().code(), code);
                assert_eq!(calls.get(), 1);
                assert_eq!(polls.get(), 0);
                assert!(!artwork.root().exists());
                assert_eq!(
                    serde_json::to_value(list_records(&kernel, &store, &artwork, None).unwrap())
                        .unwrap(),
                    before
                );
                if apply {
                    let view = kernel
                        .authorize_and_read_projection(ReadMetadataProjectionQuery::new(
                            RequestCorrelationId::new_v7(),
                            access,
                            target,
                            true,
                        ))
                        .unwrap();
                    assert!(view
                        .fields()
                        .iter()
                        .all(|field| field.resolved_field().value().is_none()));
                }
            }
        }
    }

    fn assert_committed(
        kernel: &SqliteKernel,
        access: RequestAccessContext,
        record: RecordId,
        claim: MetadataClaimId,
    ) {
        let page = kernel
            .list_records(ListRecordsQuery::new(
                RequestCorrelationId::new_v7(),
                access,
            ))
            .unwrap();
        let records = page.into_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].record_id(), record);
        assert_eq!(records[0].identifiers().len(), 1);
        assert_eq!(records[0].title().value(), Some("Original committed title"));
        let view = kernel
            .authorize_and_read_projection(ReadMetadataProjectionQuery::new(
                RequestCorrelationId::new_v7(),
                access,
                record,
                true,
            ))
            .unwrap();
        let populated = view
            .fields()
            .iter()
            .filter(|field| field.resolved_field().value().is_some())
            .collect::<Vec<_>>();
        assert_eq!(populated.len(), 1);
        assert_eq!(
            populated[0]
                .resolved_field()
                .provenance()
                .unwrap()
                .claim_id(),
            claim
        );
    }

    #[tokio::test]
    async fn optional_artwork_failure_preserves_original_create_and_apply_commit() {
        for apply in [false, true] {
            let (_root, kernel) = new_kernel();
            let store = MemoryStore::default();
            let (access, target) = prepare(&kernel, &store, apply);
            let calls = Cell::new(0);
            let polls = Cell::new(0);
            let (completed, claim) = save(&kernel, access, target, apply, "none", &calls);
            let original = *completed.as_ref().unwrap();
            let result = finish_provider_save(completed, async {
                polls.set(polls.get() + 1);
                assert_committed(&kernel, access, original, claim);
                Err(DesktopProblem::provider("Artwork unavailable"))
            })
            .await
            .unwrap();
            assert_eq!(result, original);
            assert_eq!(calls.get(), 1);
            assert_eq!(polls.get(), 1);
            assert_committed(&kernel, access, original, claim);
        }
    }

    #[tokio::test]
    async fn cancelling_optional_artwork_keeps_the_single_completed_mutation() {
        for apply in [false, true] {
            let (_root, kernel) = new_kernel();
            let store = MemoryStore::default();
            let (access, target) = prepare(&kernel, &store, apply);
            let calls = Cell::new(0);
            let polls = Cell::new(0);
            let (completed, claim) = save(&kernel, access, target, apply, "none", &calls);
            let original = *completed.as_ref().unwrap();
            let mut operation = Box::pin(finish_provider_save(completed, async {
                polls.set(polls.get() + 1);
                std::future::pending::<Result<(), DesktopProblem>>().await
            }));
            assert!(futures_util::poll!(operation.as_mut()).is_pending());
            assert_eq!(polls.get(), 1);
            drop(operation);
            assert_eq!(calls.get(), 1);
            assert_committed(&kernel, access, original, claim);
        }
    }
}
