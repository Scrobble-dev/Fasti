mod provider_identifier_details_tests {
    use super::*;
    use fasti_application::{
        AccessAdministrationPort, BrowserRequestBoundaryPolicy, BrowserSessionAccessContext,
        BrowserSessionMutationCommand, BrowserSessionPort, BrowserSessionQuery,
        CreateAuthSubjectCommand, CreateBrowserSessionCommand, CreatedBrowserSession,
        ReadProviderIdentifierDetailsRequest, RevokeCredentialCommand, ScopeKey, SecretMaterial,
        SelectBrowserSessionProfileCommand, SessionPolicy,
    };
    use fasti_domain::{
        AuthSubject, AuthSubjectId, AuthSubjectLifecycle, Grain, MembershipId, MetadataLocale,
        TrailBaseInstanceId,
    };
    use rusqlite::{params, types::Value};

    fn read_state(
        version: u64,
        status: ProviderCapabilityStatus,
        configuration: &str,
    ) -> ProviderCapabilityState {
        let search = state(version);
        ProviderCapabilityState::try_new(
            search.provider_id().clone(),
            ProviderCapabilityId::try_new("metadata.read").unwrap(),
            status,
            version,
            search.credential_requirement(),
            search.credential_reference().cloned(),
            search.credential_status(),
            ConfigurationDigest::parse(configuration.repeat(64)).unwrap(),
            ProviderCheckMetadata::never_run(),
            ProviderCheckMetadata::never_run(),
        )
        .unwrap()
    }

    fn request(
        access: ApplicationAccessContext,
        provider: &str,
        grain: Grain,
        provider_record_id: &str,
    ) -> ReadProviderIdentifierDetailsRequest {
        ReadProviderIdentifierDetailsRequest {
            correlation_id: RequestCorrelationId::new_v7(),
            access,
            provider: ProviderId::try_new(provider).unwrap(),
            grain,
            provider_record_id: provider_record_id.to_owned(),
            locale: Some(MetadataLocale::try_new("fr-FR").unwrap()),
            outbound_policy: OutboundAccessPolicy::default(),
        }
    }

    // Exact content values, not counts: a read must not rewrite existing Search
    // evidence or create action, Record, metadata, tracking, or observation rows.
    fn content_state(node: &TestNode) -> Vec<Vec<Vec<Value>>> {
        let connection = node.kernel.inner.connection.lock().unwrap();
        [
            "search_pages",
            "search_candidate_receipts",
            "search_action_receipts",
            "metadata_refresh_receipts",
            "records",
            "external_identifiers",
            "metadata_field_claims",
            "metadata_claims",
            "metadata_claim_provenance",
            "local_search_grams",
            "profile_record_tracking_dispositions",
            "metadata_profile_field_overrides",
            "observations",
        ]
        .into_iter()
        .map(|table| {
            let mut statement = connection
                .prepare(&format!("SELECT * FROM {table}"))
                .unwrap();
            let columns = statement.column_count();
            statement
                .query_map([], |row| {
                    (0..columns).map(|column| row.get(column)).collect()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<Vec<Value>>>>()
                .unwrap()
        })
        .collect()
    }

    fn copy_secret(secret: &SecretMaterial) -> SecretMaterial {
        SecretMaterial::try_from_hex(&secret.expose_hex()).unwrap()
    }

    fn browser_session(
        node: &TestNode,
        additional_grants: &[fasti_domain::ProfileGrantId],
    ) -> CreatedBrowserSession {
        let created_at = now() - Duration::seconds(15);
        let subject_id = AuthSubjectId::new_v7();
        let instance_id = TrailBaseInstanceId::new_v7();
        node.kernel
            .create_auth_subject(CreateAuthSubjectCommand::new(
                RequestCorrelationId::new_v7(),
                AuthSubject::try_new(
                    subject_id,
                    AuthSubjectLifecycle::Active,
                    1,
                    1,
                    created_at,
                    created_at,
                )
                .unwrap(),
            ))
            .unwrap();
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            connection.execute(
                "INSERT INTO trailbase_installation(singleton, trailbase_instance_id, physical_root_identity, release_lock_identity, activation_state, activation_blocker, activation_generation, created_at, updated_at) VALUES (1, ?1, ?2, ?3, 'active', NULL, 1, ?4, ?4)",
                params![
                    instance_id.to_string(),
                    Sha256Digest::from_bytes(&[31; 32]).to_string(),
                    Sha256Digest::from_bytes(&[32; 32]).to_string(),
                    timestamp(created_at)
                ],
            ).unwrap();
            connection.execute(
                "INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)",
                params![subject_id.to_string(), node.access.grant_id().to_string()],
            ).unwrap();
            for grant_id in additional_grants {
                connection.execute(
                    "INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)",
                    params![subject_id.to_string(), grant_id.to_string()],
                ).unwrap();
            }
            connection.execute(
                "INSERT INTO workspace_memberships(membership_id, auth_subject_id, workspace_id, lifecycle, role, created_at, updated_at) VALUES (?1, ?2, ?3, 'active', 'member', ?4, ?4)",
                params![
                    MembershipId::new_v7().to_string(),
                    subject_id.to_string(),
                    node.access.workspace_id().to_string(),
                    timestamp(created_at)
                ],
            ).unwrap();
        }
        let mut authorized_profile_grants = vec![node.access.grant_id()];
        authorized_profile_grants.extend_from_slice(additional_grants);
        let created = node
            .kernel
            .create_browser_session(
                CreateBrowserSessionCommand::try_new(
                    RequestCorrelationId::new_v7(),
                    subject_id,
                    node.access.workspace_id(),
                    authorized_profile_grants,
                    node.access.grant_id(),
                    SessionPolicy::try_new(
                        std::time::Duration::from_secs(120),
                        std::time::Duration::from_secs(240),
                        std::time::Duration::from_secs(480),
                        std::time::Duration::from_secs(10),
                    )
                    .unwrap(),
                    false,
                    created_at,
                )
                .unwrap(),
            )
            .unwrap();
        node.kernel.inner.connection.lock().unwrap().execute(
            "INSERT INTO fasti_browser_session_authentication(browser_session_id, trailbase_instance_id, activation_generation, method, verified_at, recent_authentication_expires_at) VALUES (?1, ?2, 1, 'trailbase_password', ?3, NULL)",
            params![
                created.session().id().to_string(),
                instance_id.to_string(),
                timestamp(created_at)
            ],
        ).unwrap();
        created
    }

    fn browser_boundary() -> BrowserRequestBoundaryPolicy {
        BrowserRequestBoundaryPolicy::try_new("https://fasti.example", "fasti.example").unwrap()
    }

    fn browser_read(created: &CreatedBrowserSession) -> ApplicationAccessContext {
        BrowserSessionAccessContext::read(
            BrowserSessionQuery::new(
                RequestCorrelationId::new_v7(),
                copy_secret(created.session_secret()),
                now(),
            ),
            browser_boundary()
                .validate_read(Some("fasti.example"))
                .unwrap(),
        )
        .into()
    }

    fn revoke_browser(node: &TestNode, created: &CreatedBrowserSession) {
        node.kernel
            .revoke_current_browser_session(BrowserSessionMutationCommand::new(
                RequestCorrelationId::new_v7(),
                copy_secret(created.session_secret()),
                copy_secret(created.csrf_secret()),
                browser_boundary()
                    .validate(Some("https://fasti.example"), Some("fasti.example"))
                    .unwrap(),
                now(),
            ))
            .unwrap();
    }

    fn select_browser_profile(
        node: &TestNode,
        created: &CreatedBrowserSession,
        grant_id: fasti_domain::ProfileGrantId,
    ) -> CreatedBrowserSession {
        node.kernel
            .select_browser_session_profile(SelectBrowserSessionProfileCommand::new(
                BrowserSessionMutationCommand::new(
                    RequestCorrelationId::new_v7(),
                    copy_secret(created.session_secret()),
                    copy_secret(created.csrf_secret()),
                    browser_boundary()
                        .validate(Some("https://fasti.example"), Some("fasti.example"))
                        .unwrap(),
                    now(),
                ),
                grant_id,
            ))
            .unwrap()
    }

    #[test]
    fn exact_coordinate_preparation_is_read_only_and_fences_authority_replacement() {
        let (node, _) = setup();
        let original = read_state(1, ProviderCapabilityStatus::Available, "a");
        node.kernel
            .put_provider_capability_state(node.access.workspace_id(), original.clone())
            .unwrap();
        let request = request(node.access.into(), "tmdb", Grain::Film, "42");
        let before = content_state(&node);

        let first = node
            .kernel
            .prepare_provider_identifier_details(&request)
            .unwrap();
        let repeated = node
            .kernel
            .prepare_provider_identifier_details(&request)
            .unwrap();
        assert_eq!(first.provider_state, original);
        assert_eq!(repeated.provider_state, first.provider_state);
        assert_eq!(
            repeated.provider_authority_fingerprint,
            first.provider_authority_fingerprint
        );
        assert_eq!(content_state(&node), before);

        node.kernel
            .put_provider_capability_state(
                node.access.workspace_id(),
                read_state(2, ProviderCapabilityStatus::Available, "b"),
            )
            .unwrap();
        let changed = node
            .kernel
            .prepare_provider_identifier_details(&request)
            .unwrap();
        assert_ne!(
            changed.provider_authority_fingerprint,
            first.provider_authority_fingerprint
        );
        node.kernel
            .put_provider_capability_state(
                node.access.workspace_id(),
                read_state(3, ProviderCapabilityStatus::Available, "a"),
            )
            .unwrap();
        let restored = node
            .kernel
            .prepare_provider_identifier_details(&request)
            .unwrap();
        assert_eq!(
            restored.provider_state.configuration_digest(),
            first.provider_state.configuration_digest()
        );
        assert_ne!(
            restored.provider_authority_fingerprint, first.provider_authority_fingerprint,
            "an away-and-back provider authority must not recreate an old fence"
        );
        assert_eq!(content_state(&node), before);
    }

    #[test]
    fn invalid_or_denied_coordinate_preparation_changes_no_content_rows() {
        let (node, _) = setup();
        node.kernel
            .put_provider_capability_state(
                node.access.workspace_id(),
                read_state(1, ProviderCapabilityStatus::Available, "a"),
            )
            .unwrap();
        let before = content_state(&node);
        for (provider, grain, value) in [
            ("tmdb", Grain::Film, "0"),
            ("tmdb", Grain::Film, "042"),
            ("tmdb", Grain::Film, "not-a-number"),
            ("tmdb", Grain::Episode, "42"),
            ("google-books", Grain::Film, "42"),
        ] {
            assert_eq!(
                node.kernel
                    .prepare_provider_identifier_details(&request(
                        node.access.into(),
                        provider,
                        grain,
                        value,
                    ))
                    .unwrap_err()
                    .code(),
                ProblemCode::ValidationFailed,
                "{provider}/{grain:?}/{value}"
            );
            assert_eq!(content_state(&node), before);
        }

        let mut malformed_policy = request(node.access.into(), "tmdb", Grain::Film, "42");
        malformed_policy.outbound_policy.deny_providers = vec!["TMDB".into()];
        assert_eq!(
            node.kernel
                .prepare_provider_identifier_details(&malformed_policy)
                .unwrap_err()
                .code(),
            ProblemCode::Forbidden
        );
        assert_eq!(content_state(&node), before);

        node.kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM grant_scopes WHERE grant_id = ?1 AND scope_key = 'metadata_search'",
                [node.access.grant_id().to_string()],
            )
            .unwrap();
        let denied_baseline = content_state(&node);
        assert_eq!(
            node.kernel
                .prepare_provider_identifier_details(&request(
                    node.access.into(),
                    "tmdb",
                    Grain::Film,
                    "42",
                ))
                .unwrap_err()
                .code(),
            ProblemCode::Forbidden
        );
        assert_eq!(content_state(&node), denied_baseline);
    }

    #[test]
    fn browser_profile_rotation_changes_only_the_authorized_access_fence() {
        let (node, _) = setup();
        node.kernel
            .put_provider_capability_state(
                node.access.workspace_id(),
                read_state(1, ProviderCapabilityStatus::Available, "a"),
            )
            .unwrap();
        let alternate = node.add_profile_with_scopes(&[ScopeKey::MetadataSearch]);
        let created = browser_session(&node, &[alternate.grant_id()]);
        let before = content_state(&node);
        let first = node
            .kernel
            .prepare_provider_identifier_details(&request(
                browser_read(&created),
                "tmdb",
                Grain::Film,
                "42",
            ))
            .unwrap();

        let selected = select_browser_profile(&node, &created, alternate.grant_id());
        let rotated = node
            .kernel
            .prepare_provider_identifier_details(&request(
                browser_read(&selected),
                "tmdb",
                Grain::Film,
                "42",
            ))
            .unwrap();

        assert_ne!(rotated.authorized_access, first.authorized_access);
        assert_eq!(
            rotated.authorized_access.workspace_id(),
            node.access.workspace_id()
        );
        assert_eq!(rotated.authorized_access.grant_id(), alternate.grant_id());
        assert_eq!(rotated.provider_state, first.provider_state);
        assert_eq!(
            rotated.provider_authority_fingerprint,
            first.provider_authority_fingerprint
        );
        assert_eq!(content_state(&node), before);
    }

    #[test]
    fn bearer_and_browser_reads_recheck_current_revocation_without_content_writes() {
        for browser in [false, true] {
            let (node, _) = setup();
            node.kernel
                .put_provider_capability_state(
                    node.access.workspace_id(),
                    read_state(1, ProviderCapabilityStatus::Available, "a"),
                )
                .unwrap();
            let created = browser.then(|| browser_session(&node, &[]));
            let access = created
                .as_ref()
                .map_or_else(|| node.access.into(), browser_read);
            let request = request(access, "tmdb", Grain::Film, "42");
            let before = content_state(&node);
            node.kernel
                .prepare_provider_identifier_details(&request)
                .unwrap();
            assert_eq!(content_state(&node), before);

            if let Some(created) = &created {
                revoke_browser(&node, created);
            } else {
                node.kernel
                    .revoke_credential(RevokeCredentialCommand::new(
                        RequestCorrelationId::new_v7(),
                        node.access,
                        node.access.credential_id(),
                    ))
                    .unwrap();
            }
            let revoked_baseline = content_state(&node);
            assert_eq!(
                node.kernel
                    .prepare_provider_identifier_details(&request)
                    .unwrap_err()
                    .code(),
                if browser {
                    ProblemCode::BrowserSessionRevoked
                } else {
                    ProblemCode::Forbidden
                }
            );
            assert_eq!(content_state(&node), revoked_baseline);
        }
    }
}
