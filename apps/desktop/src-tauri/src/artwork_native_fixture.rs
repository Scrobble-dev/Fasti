#[cfg(all(target_os = "linux", feature = "desktop-runtime"))]
mod native_fixture {
    use super::*;
    use crate::{records, secure_storage, setup};
    use fasti_application::{
        provider_identity_mapping, ApplyProviderMetadataCommand, ConfigureMetadataProjectionCommand,
        CreateRecordCommand, IdentityPort, MetadataProjectionPort, ProviderMetadataField,
        ProviderMetadataPort, RegisterNamespaceDefinitionCommand,
    };
    use fasti_domain::{
        FieldClaim, FieldClaimProvenance, FieldClaimStatus, FieldKey, Grain, MetadataClaimId,
        MetadataFieldGroup, MetadataProjectionPolicy, MetadataProviderId, NamespaceKey, ReceivedAt,
        RequestCorrelationId, Sha256Digest,
    };
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    const MARKER: &[u8] = b"disposable native artwork fixture\n";
    const POSTER: &str = "https://image.tmdb.org/t/p/w500/fasti-native-artwork-fixture.png";
    const TITLE: &str = "Synthetic native artwork fixture";
    const PNG: &[u8] = include_bytes!("../icons/icon.png");

    fn required_path(name: &str) -> PathBuf {
        let path = PathBuf::from(std::env::var_os(name).expect("required fixture path is missing"));
        assert!(path.is_absolute(), "fixture paths must be absolute");
        assert!(
            fs::canonicalize(&path).expect("fixture path must exist") == path,
            "fixture paths must be canonical and contain no symlink components",
        );
        path
    }

    fn private_directory(path: &Path, owner: u32) {
        let metadata = fs::symlink_metadata(path).expect("fixture directory must exist");
        assert!(metadata.file_type().is_dir(), "fixture directory must not be a symlink");
        assert!(metadata.uid() == owner, "fixture directory must belong to this process owner");
        assert!(metadata.mode() & 0o777 == 0o700, "fixture directory must have mode 0700");
    }

    #[test]
    #[ignore = "requires an explicitly marked disposable fixture and private D-Bus/keyring"]
    fn seed_native_artwork_fixture() {
        let root = required_path("FASTI_NATIVE_ARTWORK_FIXTURE_ROOT");
        let data = required_path("FASTI_DATA_ROOT");
        let cache_home = required_path("XDG_CACHE_HOME");
        assert!(data == root.join("data"), "data must be the exact fixture data child");
        assert!(cache_home == root.join("cache"), "cache must be the exact fixture cache child");
        let owner = fs::metadata("/proc/self").expect("Linux process metadata").uid();
        for path in [&root, &data, &cache_home] {
            private_directory(path, owner);
        }
        for path in [&data, &cache_home] {
            assert!(
                fs::read_dir(path).expect("read disposable directory").next().is_none(),
                "fixture data and cache directories must be empty",
            );
        }
        let marker_path = root.join(".fasti-native-artwork-fixture");
        let mut marker = open_cache_image(&marker_path).expect("private regular marker is required");
        let metadata = marker.metadata().expect("marker metadata");
        assert!(metadata.uid() == owner, "marker must belong to this process owner");
        assert!(metadata.mode() & 0o077 == 0, "marker must be private");
        assert!(metadata.nlink() == 1, "marker must not be hard-linked");
        assert!(metadata.len() == MARKER.len() as u64, "marker length differs");
        let mut marker_bytes = vec![0; MARKER.len()];
        marker.read_exact(&mut marker_bytes).expect("read bounded marker");
        assert!(marker_bytes == MARKER, "disposable fixture marker differs");
        assert!(
            fs::symlink_metadata(root.join("artwork-fixture.json"))
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
            "fixture receipt must not already exist",
        );
        assert!(
            std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some(),
            "the harness must provide its private D-Bus/keyring environment",
        );
        assert!(std::env::var_os("FASTI_TRAILBASE_ROOT").is_none(), "TrailBase is outside this fixture");
        assert_eq!(image_dimensions(PNG), Some((512, 512)));
        assert_eq!(image_content_type(PNG), Some("image/png"));

        // Use the real scoped platform store, not MemoryStore or an invented
        // access context. The harness owns the private bus/keyring lifecycle.
        secure_storage::initialize().expect("initialize platform credential storage");
        let kernel = fasti_store::SqliteKernel::open(&data).expect("open disposable node");
        let secrets = setup::KeyringSetupSecretStore::new(kernel.data_root_identity());
        setup::complete_setup(&kernel, &secrets).expect("initialize and enroll disposable node");
        let access = records::require_access(&kernel, &secrets).expect("authenticate scoped fixture");
        kernel
            .authorize_and_configure_projection(ConfigureMetadataProjectionCommand::new(
                RequestCorrelationId::new_v7(),
                access,
                MetadataProjectionPolicy::default_for_profile(access.profile_id()),
                None,
                vec![MetadataFieldGroup::BasicInfo],
                Vec::new(),
            ))
            .expect("configure disposable profile projection");
        let mapping = provider_identity_mapping(TMDB_PROVIDER, "movie").expect("existing mapping");
        kernel
            .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                RequestCorrelationId::new_v7(),
                access,
                mapping.namespace_definition().expect("existing namespace definition"),
            ))
            .expect("register fixture namespace");
        let record = kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                access,
                Grain::Film,
            ))
            .expect("create fixture Record")
            .record_id();
        let observed = chrono::Utc::now();
        let expires = observed + chrono::Duration::minutes(10);
        let policy = ProviderResponseCachePolicy::new(
            ProviderResponseReuse::Reusable,
            observed,
            Duration::ZERO,
            Some(Duration::from_secs(600)),
            Some(Duration::ZERO),
        );
        // These rows are explicitly synthetic cached evidence. No provider
        // request, health change, credential, or upstream success is asserted.
        let fixture_bytes: [u8; 32] = Sha256::digest(
            b"Fasti synthetic native artwork fixture; no provider response",
        )
        .into();
        let fixture_digest = Sha256Digest::from_bytes(&fixture_bytes);
        let provenance = FieldClaimProvenance::try_new(
            MetadataProviderId::try_new(TMDB_PROVIDER).unwrap(),
            NamespaceKey::try_new("tmdb.movie").unwrap(),
            "42",
            None,
            None,
            Some("synthetic-native-artwork-fixture.v1".to_owned()),
            fixture_digest,
        )
        .expect("explicit fixture provenance");
        let fields = [("core.title", TITLE), ("core.poster_url", POSTER)]
            .into_iter()
            .map(|(key, value)| {
                ProviderMetadataField::new(
                    FieldKey::try_new(key).unwrap(),
                    FieldClaim::try_new_unbound_provider(
                        MetadataClaimId::new_v7(),
                        value,
                        provenance.clone(),
                        ReceivedAt::from_application_clock(observed),
                        Some(expires),
                        FieldClaimStatus::Fresh,
                    )
                    .expect("bounded fixture claim"),
                )
            })
            .collect();
        kernel
            .apply_provider_metadata(ApplyProviderMetadataCommand::new(
                RequestCorrelationId::new_v7(),
                access,
                record,
                mapping.identifier("42").expect("fixture identifier"),
                fields,
                policy,
            ))
            .expect("apply fixture metadata using existing owner");
        let artwork = ArtworkCache::new(
            cache_home.join("dev.scrobble.fasti/provider-artwork"),
            kernel.data_root_identity(),
        );
        artwork.store(TMDB_PROVIDER, POSTER, PNG, &policy).expect("store real PNG fixture envelope");
        let page = records::list_records(
            &kernel,
            &secrets,
            &artwork,
            Some(fasti_contracts::ListRecordsQueryParameters {
                record_id: Some(record.to_string()),
            }),
        )
        .expect("read actual Desktop Record DTO");
        let page = serde_json::to_value(page).expect("serialize Desktop DTO");
        assert_eq!(page["truncated"], false);
        let rows = page["records"].as_array().expect("Record rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["record_id"], record.to_string());
        assert_eq!(rows[0]["title"]["value"], TITLE);
        let locator = rows[0]["poster_asset_path"].as_str().expect("real native artwork locator");
        assert_eq!(artwork.locator_record(locator, access), Some(record));
        assert_eq!(artwork.cached_entry(TMDB_PROVIDER, POSTER).unwrap().bytes, PNG);

        let digest = format!("sha256:{:x}", Sha256::digest(PNG));
        let receipt = serde_json::json!({
            "record_id": record.to_string(),
            "locator": locator,
            "image_digest": digest,
            "width": 512,
            "height": 512,
            "expires_at": expires.to_rfc3339(),
        });
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(root.join("artwork-fixture.json"))
            .expect("create private fixture receipt without overwrite");
        serde_json::to_writer(&mut file, &receipt).expect("write non-secret fixture receipt");
        file.write_all(b"\n").expect("finish receipt");
        file.sync_all().expect("sync fixture receipt");
        open_cache_directory(&root).expect("open fixture root").sync_all().expect("sync fixture root");
        drop(artwork);
        drop(kernel);
    }
}
