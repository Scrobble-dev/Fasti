mod policy_tests {
    use super::*;
    use crate::setup::test_support::new_kernel;
    use chrono::{DateTime, TimeDelta, Utc};

    const URL: &str = "https://image.tmdb.org/t/p/w500/policy-fixture.png";

    // Cache/envelope fixture only: this is the existing image header
    // classifier's bounded PNG input, not a decoded image or live HTTP proof.
    fn image() -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&500_u32.to_be_bytes());
        bytes.extend_from_slice(&750_u32.to_be_bytes());
        bytes
    }

    fn observed() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-05T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn policy(mode: ProviderResponseReuse) -> ProviderResponseCachePolicy {
        ProviderResponseCachePolicy::new(
            mode,
            observed(),
            Duration::from_secs(10),
            Some(Duration::from_secs(60)),
            Some(Duration::from_secs(30)),
        )
    }

    fn envelope(header: &[u8], body: &[u8]) -> Vec<u8> {
        let mut bytes = (header.len() as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(header);
        bytes.extend_from_slice(body);
        bytes
    }

    fn decode_header(bytes: &[u8]) -> ArtworkHeader {
        let size = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        serde_json::from_slice(&bytes[4..4 + size]).unwrap()
    }

    #[cfg(feature = "desktop-runtime")]
    mod response_lifecycle {
        use super::*;
        use std::cell::Cell;

        fn reply(cache_control: &str, content_type: &str, body: Vec<u8>) -> reqwest::Response {
            // In-memory HTTP transport fixture: exercise the real reqwest
            // response/body adapter, without network or provider-health claims.
            reqwest::Response::from(
                tauri::http::Response::builder()
                    .status(200)
                    .header("cache-control", cache_control)
                    .header("content-type", content_type)
                    .body(body)
                    .unwrap(),
            )
        }

        fn stale_policy(mode: ProviderResponseReuse, grace: u64) -> ProviderResponseCachePolicy {
            ProviderResponseCachePolicy::new(
                mode,
                Utc::now() - TimeDelta::seconds(120),
                Duration::ZERO,
                Some(Duration::from_secs(30)),
                Some(Duration::from_secs(grace)),
            )
        }

        #[tokio::test]
        async fn artwork_delayed_body_retains_header_time_and_guard_until_completion_or_cancel() {
            use std::sync::{atomic::AtomicBool, Arc};

            struct RequestGuard(Arc<AtomicBool>);
            impl Drop for RequestGuard {
                fn drop(&mut self) {
                    self.0.store(true, Ordering::SeqCst);
                }
            }

            let (root, kernel) = new_kernel();
            let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
            for cancel in [false, true] {
                cache
                    .store(
                        TMDB_PROVIDER,
                        URL,
                        &image(),
                        &stale_policy(ProviderResponseReuse::Reusable, 600),
                    )
                    .unwrap();
                let path = cache.cache_path(TMDB_PROVIDER, URL).unwrap();
                let (body_started, started) = tokio::sync::oneshot::channel();
                let (release_body, released) = tokio::sync::oneshot::channel();
                let guard_dropped = Arc::new(AtomicBool::new(false));
                let guard = RequestGuard(Arc::clone(&guard_dropped));
                let body = reqwest::Body::wrap_stream(futures_util::stream::once(async move {
                    body_started.send(()).unwrap();
                    released.await.map_err(|_| std::io::Error::other("fixture body cancelled"))?;
                    Ok::<_, std::io::Error>(image())
                }));
                let response = reqwest::Response::from(
                    tauri::http::Response::builder()
                        .status(200)
                        .header("content-type", "image/png")
                        .header("cache-control", "max-age=600")
                        .body(body)
                        .unwrap(),
                );
                let before_request = Utc::now();
                let mut pending = Box::pin(cache.load_with_response(TMDB_PROVIDER, URL, async {
                    assert!(!path.exists(), "old envelope must be gone before request polling");
                    Ok((response, Duration::from_millis(15), guard))
                }));
                tokio::select! {
                    result = &mut pending => panic!("body completed before release: {}", result.is_ok()),
                    ready = started => ready.unwrap(),
                    _ = tokio::time::sleep(Duration::from_secs(5)) => panic!("body was not polled"),
                }
                assert!(!guard_dropped.load(Ordering::SeqCst));
                assert!(!path.exists());
                assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none());
                if cancel {
                    drop(pending);
                    assert!(guard_dropped.load(Ordering::SeqCst));
                    assert!(release_body.send(()).is_err(), "cancel must drop the pending body");
                    assert!(!path.exists());
                    assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none());
                } else {
                    // Separate receipt time from completion time without
                    // relying on image decoding, network, or a fake clock.
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    let body_release_time = Utc::now();
                    assert!(!guard_dropped.load(Ordering::SeqCst));
                    release_body.send(()).unwrap();
                    let returned = tokio::time::timeout(Duration::from_secs(5), pending)
                        .await
                        .expect("bounded body completion")
                        .unwrap();
                    assert_eq!(returned, image());
                    assert!(guard_dropped.load(Ordering::SeqCst));
                    let entry = cache.cached_entry(TMDB_PROVIDER, URL).unwrap();
                    assert!(entry.header.policy.received_at() >= before_request);
                    assert!(entry.header.policy.received_at() < body_release_time);
                    assert_eq!(entry.bytes, image());
                }
            }
        }

        #[tokio::test]
        async fn artwork_fresh_cache_hit_does_not_poll_the_request_future() {
            let (root, kernel) = new_kernel();
            let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
            let policy = ProviderResponseCachePolicy::new(
                ProviderResponseReuse::Reusable,
                Utc::now() - TimeDelta::seconds(1),
                Duration::ZERO,
                Some(Duration::from_secs(600)),
                None,
            );
            cache.store(TMDB_PROVIDER, URL, &image(), &policy).unwrap();
            let original = fs::read(cache.cache_path(TMDB_PROVIDER, URL).unwrap()).unwrap();
            let polled = Cell::new(false);
            let bytes = cache
                .load_with_response::<()>(TMDB_PROVIDER, URL, async {
                    polled.set(true);
                    Err(DesktopProblem::provider("unexpected request"))
                })
                .await
                .unwrap();
            assert_eq!(bytes, image());
            assert!(!polled.get());
            assert_eq!(fs::read(cache.cache_path(TMDB_PROVIDER, URL).unwrap()).unwrap(), original);
        }

        #[tokio::test]
        async fn artwork_request_error_restores_only_eligible_original_stale_evidence() {
            let (root, kernel) = new_kernel();
            let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
            let policy = stale_policy(ProviderResponseReuse::Reusable, 600);
            cache.store(TMDB_PROVIDER, URL, &image(), &policy).unwrap();
            let entry = cache.cached_entry(TMDB_PROVIDER, URL).unwrap();
            assert!(!entry.reusable_at(Utc::now(), false));
            assert!(entry.reusable_at(Utc::now(), true));
            let path = cache.cache_path(TMDB_PROVIDER, URL).unwrap();
            let original = fs::read(&path).unwrap();
            let polled = Cell::new(false);
            let bytes = cache
                .load_with_response::<()>(TMDB_PROVIDER, URL, async {
                    polled.set(true);
                    assert!(!path.exists(), "invalidate must complete before request polling");
                    assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none());
                    Err(DesktopProblem::provider("fixture transport failure"))
                })
                .await
                .unwrap();
            assert!(polled.get());
            assert_eq!(bytes, image());
            assert_eq!(fs::read(path).unwrap(), original);
            assert_eq!(cache.cached_entry(TMDB_PROVIDER, URL).unwrap().header.policy, policy);
        }

        #[tokio::test]
        #[cfg(unix)]
        async fn artwork_invalidation_permission_failure_never_polls_and_keeps_stale_policy() {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};

            struct RestoreDirectoryPermissions(File);
            impl Drop for RestoreDirectoryPermissions {
                fn drop(&mut self) {
                    let _ = self.0.set_permissions(fs::Permissions::from_mode(0o700));
                }
            }

            let (root, kernel) = new_kernel();
            if fs::metadata(root.path()).unwrap().uid() == 0 {
                eprintln!("read-only directory fixture is not enforced for root; skipping");
                return;
            }
            let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
            for (mode, grace, allowed) in [
                (ProviderResponseReuse::Reusable, 600, true),
                (ProviderResponseReuse::Reusable, 60, false),
                (ProviderResponseReuse::ValidateWhenStale, 600, false),
                (ProviderResponseReuse::ValidateEveryReuse, 600, false),
            ] {
                let policy = stale_policy(mode, grace);
                cache.store(TMDB_PROVIDER, URL, &image(), &policy).unwrap();
                let path = cache.cache_path(TMDB_PROVIDER, URL).unwrap();
                let original = fs::read(&path).unwrap();
                let restore = RestoreDirectoryPermissions(File::open(cache.cache_root().unwrap()).unwrap());
                restore.0.set_permissions(fs::Permissions::from_mode(0o500)).unwrap();
                let polled = Cell::new(false);
                let result = cache
                    .load_with_response::<()>(TMDB_PROVIDER, URL, async {
                        polled.set(true);
                        Err(DesktopProblem::provider("request must not run after failed invalidation"))
                    })
                    .await;
                let after = fs::read(&path);
                // Restore even on unwinding, before TempDir attempts cleanup.
                drop(restore);
                assert!(!polled.get(), "{mode:?}");
                match result {
                    Ok(bytes) => {
                        assert!(allowed, "{mode:?}");
                        assert_eq!(bytes, image());
                    }
                    Err(_) => assert!(!allowed, "eligible stale image must survive local write failure"),
                }
                assert_eq!(after.unwrap(), original);
                assert_eq!(cache.cached_entry(TMDB_PROVIDER, URL).unwrap().header.policy, policy);
                assert_eq!(fs::read_dir(cache.root()).unwrap().count(), 1);
            }
        }

        #[tokio::test]
        async fn artwork_request_error_does_not_restore_expired_or_revalidation_required_images() {
            let (root, kernel) = new_kernel();
            let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
            for (mode, grace) in [
                (ProviderResponseReuse::Reusable, 60),
                (ProviderResponseReuse::ValidateWhenStale, 600),
                (ProviderResponseReuse::ValidateEveryReuse, 600),
            ] {
                cache.store(TMDB_PROVIDER, URL, &image(), &stale_policy(mode, grace)).unwrap();
                let entry = cache.cached_entry(TMDB_PROVIDER, URL).unwrap();
                assert!(!entry.reusable_at(Utc::now(), false));
                assert!(!entry.reusable_at(Utc::now(), true));
                let path = cache.cache_path(TMDB_PROVIDER, URL).unwrap();
                let polled = Cell::new(false);
                let result = cache
                    .load_with_response::<()>(TMDB_PROVIDER, URL, async {
                        polled.set(true);
                        assert!(!path.exists());
                        assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none());
                        Err(DesktopProblem::provider("fixture transport failure"))
                    })
                    .await;
                assert!(polled.get());
                assert!(result.is_err(), "{mode:?}");
                assert!(!path.exists());
                assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none());
            }
        }

        #[tokio::test]
        async fn artwork_no_store_response_delivers_live_bytes_without_old_or_new_disk_reuse() {
            let (root, kernel) = new_kernel();
            let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
            cache.store(TMDB_PROVIDER, URL, &image(), &stale_policy(ProviderResponseReuse::Reusable, 600)).unwrap();
            let path = cache.cache_path(TMDB_PROVIDER, URL).unwrap();
            let mut live = image();
            live[16..20].copy_from_slice(&300_u32.to_be_bytes());
            let returned = cache
                .load_with_response(TMDB_PROVIDER, URL, async {
                    assert!(!path.exists());
                    assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none());
                    Ok((reply("no-store", "image/png", live.clone()), Duration::ZERO, ()))
                })
                .await
                .unwrap();
            assert_eq!(returned, live);
            assert!(!path.exists());
            assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none());
            assert_eq!(fs::read_dir(cache.root()).unwrap().count(), 0);
        }

        #[tokio::test]
        async fn artwork_no_cache_response_is_retained_but_next_load_requires_a_new_request() {
            let (root, kernel) = new_kernel();
            let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
            cache.store(TMDB_PROVIDER, URL, &image(), &stale_policy(ProviderResponseReuse::Reusable, 600)).unwrap();
            let path = cache.cache_path(TMDB_PROVIDER, URL).unwrap();
            let before = Utc::now();
            let returned = cache
                .load_with_response(TMDB_PROVIDER, URL, async {
                    assert!(!path.exists());
                    Ok((reply("no-cache, max-age=600", "image/png", image()), Duration::from_millis(15), ()))
                })
                .await
                .unwrap();
            assert_eq!(returned, image());
            let entry = cache.cached_entry(TMDB_PROVIDER, URL).unwrap();
            assert_eq!(entry.header.policy.reuse(), ProviderResponseReuse::ValidateEveryReuse);
            assert!(entry.header.policy.received_at() >= before);
            assert!(entry.header.policy.received_at() <= Utc::now());
            assert!(!entry.reusable_at(entry.header.policy.received_at(), false));
            assert!(!entry.reusable_at(Utc::now(), true));
            let polled = Cell::new(false);
            let result = cache
                .load_with_response::<()>(TMDB_PROVIDER, URL, async {
                    polled.set(true);
                    assert!(!path.exists());
                    assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none());
                    Err(DesktopProblem::provider("second request failed"))
                })
                .await;
            assert!(polled.get());
            assert!(result.is_err());
            assert!(!path.exists());
        }

        #[tokio::test]
        async fn artwork_observed_invalid_200_response_never_resurrects_stale_bytes() {
            let (root, kernel) = new_kernel();
            let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
            for defect in ["mime", "body", "oversized"] {
                cache.store(TMDB_PROVIDER, URL, &image(), &stale_policy(ProviderResponseReuse::Reusable, 600)).unwrap();
                assert!(cache.cached_entry(TMDB_PROVIDER, URL).unwrap().reusable_at(Utc::now(), true));
                let path = cache.cache_path(TMDB_PROVIDER, URL).unwrap();
                let mut body = image();
                let mime = match defect {
                    "mime" => "text/html",
                    "body" => {
                        body = b"not an image".to_vec();
                        "image/png"
                    }
                    "oversized" => {
                        body.resize(ARTWORK_LIMIT + 1, 0);
                        "image/png"
                    }
                    _ => unreachable!(),
                };
                let result = cache
                    .load_with_response(TMDB_PROVIDER, URL, async {
                        assert!(!path.exists());
                        assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none());
                        Ok((reply("max-age=600", mime, body), Duration::ZERO, ()))
                    })
                    .await;
                assert!(result.is_err(), "{defect}");
                assert!(!path.exists(), "{defect}");
                assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none(), "{defect}");
            }
        }
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn artwork_retained_directory_survives_path_rename_and_replacement() {
        use std::os::unix::fs::MetadataExt;

        let (root, kernel) = new_kernel();
        let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
        let policy = policy(ProviderResponseReuse::Reusable);
        let body = image();
        cache.store(TMDB_PROVIDER, URL, &body, &policy).unwrap();
        let configured = cache.root().to_path_buf();
        let retained = cache.cache_root().unwrap();
        let opened = fs::metadata(&retained).unwrap();
        let name = cache.path_for(TMDB_PROVIDER, URL).file_name().unwrap().to_owned();
        let original_envelope = fs::read(configured.join(&name)).unwrap();
        let moved = root.path().join("moved-open-artwork");
        fs::rename(&configured, &moved).unwrap();
        fs::create_dir(&configured).unwrap();
        let replacement_bytes = b"replacement contents must not be read or overwritten";
        fs::write(configured.join(&name), replacement_bytes).unwrap();
        fs::write(configured.join("replacement-marker"), b"untouched").unwrap();

        cache.prepare().unwrap();
        assert_eq!(cache.cache_root().unwrap(), retained);
        let still_opened = fs::metadata(cache.cache_root().unwrap()).unwrap();
        assert_eq!((still_opened.dev(), still_opened.ino()), (opened.dev(), opened.ino()));
        assert_ne!(fs::metadata(&configured).unwrap().ino(), opened.ino());
        assert_eq!(cache.cached_entry(TMDB_PROVIDER, URL).unwrap().bytes, body);
        assert_eq!(fs::read(moved.join(&name)).unwrap(), original_envelope);

        let mut changed = image();
        changed[16..20].copy_from_slice(&400_u32.to_be_bytes());
        cache.store(TMDB_PROVIDER, URL, &changed, &policy).unwrap();
        assert_eq!(cache.cached_entry(TMDB_PROVIDER, URL).unwrap().bytes, changed);
        assert_eq!(
            decode_header(&fs::read(moved.join(&name)).unwrap()).digest,
            <[u8; 32]>::from(Sha256::digest(&changed)),
        );
        assert_eq!(fs::read(configured.join(&name)).unwrap(), replacement_bytes);

        let second = "https://image.tmdb.org/t/p/w500/after-directory-replacement.png";
        let second_name = cache.path_for(TMDB_PROVIDER, second).file_name().unwrap().to_owned();
        cache.store(TMDB_PROVIDER, second, &body, &policy).unwrap();
        assert!(moved.join(&second_name).is_file());
        assert!(!configured.join(&second_name).exists());
        assert_eq!(cache.cached_entry(TMDB_PROVIDER, second).unwrap().bytes, body);

        cache.invalidate(TMDB_PROVIDER, URL).unwrap();
        assert!(!moved.join(&name).exists());
        assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none());
        assert!(moved.join(&second_name).is_file());
        assert_eq!(fs::read(configured.join(&name)).unwrap(), replacement_bytes);
        assert_eq!(fs::read(configured.join("replacement-marker")).unwrap(), b"untouched");
        assert_eq!(fs::read_dir(&configured).unwrap().count(), 2);
    }

    #[test]
    fn artwork_policy_envelope_preserves_original_observation_across_real_reopen() {
        let (root, kernel) = new_kernel();
        let cache_root = root.path().join("artwork");
        let cache = ArtworkCache::new(&cache_root, kernel.data_root_identity());
        let policy = policy(ProviderResponseReuse::Reusable);
        let body = image();
        cache.store(TMDB_PROVIDER, URL, &body, &policy).unwrap();
        let path = cache.path_for(TMDB_PROVIDER, URL);
        let original = fs::read(&path).unwrap();
        let first = cache.cached_entry(TMDB_PROVIDER, URL).unwrap();
        assert_eq!(first.header.policy, policy);
        assert_eq!(first.bytes, body);
        assert_eq!(first.header.length as usize, body.len());
        assert_eq!(first.header.digest, <[u8; 32]>::from(Sha256::digest(&body)));
        assert_eq!(first.header.key, path.file_name().unwrap().to_str().unwrap());
        drop(cache);
        drop(kernel);

        let reopened = fasti_store::SqliteKernel::open(root.path()).unwrap();
        let cache = ArtworkCache::new(&cache_root, reopened.data_root_identity());
        assert_eq!(cache.path_for(TMDB_PROVIDER, URL), path);
        let restored = cache.cached_entry(TMDB_PROVIDER, URL).unwrap();
        assert_eq!(restored.header.policy, policy);
        assert_eq!(restored.bytes, body);
        assert!(restored.reusable_at(observed() + TimeDelta::seconds(49), false));
        assert!(!restored.reusable_at(observed() + TimeDelta::seconds(50), false));
        assert!(restored.reusable_at(observed() + TimeDelta::seconds(79), true));
        assert!(!restored.reusable_at(observed() + TimeDelta::seconds(80), true));
        assert_eq!(fs::read(path).unwrap(), original);
    }

    #[test]
    fn artwork_no_store_is_rejected_without_writing_or_replacing_an_envelope() {
        let (root, kernel) = new_kernel();
        let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
        let body = image();
        let forbidden = policy(ProviderResponseReuse::NoStore);
        assert!(cache.store(TMDB_PROVIDER, URL, &body, &forbidden).is_err());
        assert!(!cache.root().exists());
        assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none());

        cache
            .store(TMDB_PROVIDER, URL, &body, &policy(ProviderResponseReuse::Reusable))
            .unwrap();
        let path = cache.path_for(TMDB_PROVIDER, URL);
        let before = fs::read(&path).unwrap();
        assert!(cache.store(TMDB_PROVIDER, URL, &body, &forbidden).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        // A restrictive network observation must invalidate via the fetch
        // owner. Direct store rejection is deliberately not that observation.
        assert_eq!(fs::read_dir(cache.root()).unwrap().count(), 1);
    }

    #[test]
    fn artwork_reuse_enforces_exact_fresh_stale_and_backward_clock_boundaries() {
        let (root, kernel) = new_kernel();
        let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
        let tick = TimeDelta::nanoseconds(1);
        for mode in [
            ProviderResponseReuse::Reusable,
            ProviderResponseReuse::ValidateWhenStale,
            ProviderResponseReuse::ValidateEveryReuse,
        ] {
            let policy = policy(mode);
            cache.store(TMDB_PROVIDER, URL, &image(), &policy).unwrap();
            let original = fs::read(cache.path_for(TMDB_PROVIDER, URL)).unwrap();
            let entry = cache.cached_entry(TMDB_PROVIDER, URL).unwrap();
            let fresh = observed() + TimeDelta::seconds(50);
            let stale = observed() + TimeDelta::seconds(80);
            for on_error in [false, true] {
                assert!(!entry.reusable_at(observed() - tick, on_error));
                assert_eq!(
                    entry.reusable_at(observed(), on_error),
                    mode != ProviderResponseReuse::ValidateEveryReuse,
                );
                assert_eq!(
                    entry.reusable_at(fresh - tick, on_error),
                    mode != ProviderResponseReuse::ValidateEveryReuse,
                );
                let stale_permitted = on_error && mode == ProviderResponseReuse::Reusable;
                assert_eq!(entry.reusable_at(fresh, on_error), stale_permitted);
                assert_eq!(entry.reusable_at(stale - tick, on_error), stale_permitted);
                assert!(!entry.reusable_at(stale, on_error));
            }
            assert_eq!(fs::read(cache.path_for(TMDB_PROVIDER, URL)).unwrap(), original);
            if mode == ProviderResponseReuse::ValidateEveryReuse {
                // Retained evidence is not a cache hit, even at observation.
                assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_some());
                assert!(cache.local_path(TMDB_PROVIDER, URL).is_none());
            }
        }
    }

    #[test]
    fn artwork_reuse_caps_missing_or_long_upstream_lifetimes_without_renewal() {
        let (root, kernel) = new_kernel();
        let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
        for (freshness, grace) in [
            (None, None),
            (Some(Duration::MAX), Some(Duration::MAX)),
        ] {
            let policy = ProviderResponseCachePolicy::new(
                ProviderResponseReuse::Reusable,
                observed(),
                Duration::ZERO,
                freshness,
                grace,
            );
            cache.store(TMDB_PROVIDER, URL, &image(), &policy).unwrap();
            let entry = cache.cached_entry(TMDB_PROVIDER, URL).unwrap();
            let fresh = observed() + TimeDelta::from_std(IMAGE_FRESH_CAP).unwrap();
            let stale = observed() + TimeDelta::from_std(IMAGE_STALE_CAP).unwrap();
            assert!(entry.reusable_at(fresh - TimeDelta::nanoseconds(1), false));
            assert!(!entry.reusable_at(fresh, false));
            assert!(entry.reusable_at(stale - TimeDelta::nanoseconds(1), true));
            assert!(!entry.reusable_at(stale, true));
            assert_eq!(entry.header.policy.received_at(), observed());
        }
        let aged_out = ProviderResponseCachePolicy::new(
            ProviderResponseReuse::Reusable,
            observed(),
            Duration::from_secs(90),
            Some(Duration::from_secs(60)),
            Some(Duration::from_secs(30)),
        );
        cache.store(TMDB_PROVIDER, URL, &image(), &aged_out).unwrap();
        let entry = cache.cached_entry(TMDB_PROVIDER, URL).unwrap();
        assert!(!entry.reusable_at(observed(), false));
        assert!(!entry.reusable_at(observed(), true));
    }

    #[test]
    fn artwork_envelope_rejects_misbound_evidence_lengths_and_body_corruption() {
        let (root, kernel) = new_kernel();
        let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
        let body = image();
        cache
            .store(TMDB_PROVIDER, URL, &body, &policy(ProviderResponseReuse::Reusable))
            .unwrap();
        let path = cache.path_for(TMDB_PROVIDER, URL);
        let original = fs::read(&path).unwrap();
        for defect in ["version", "key", "digest", "zero", "short", "long", "oversized", "no_store"] {
            let mut header = decode_header(&original);
            match defect {
                "version" => header.version = 2,
                "key" => header.key = "00".repeat(32),
                "digest" => header.digest[0] ^= 1,
                "zero" => header.length = 0,
                "short" => header.length -= 1,
                "long" => header.length += 1,
                "oversized" => header.length = ARTWORK_LIMIT as u32 + 1,
                "no_store" => header.policy = policy(ProviderResponseReuse::NoStore),
                _ => unreachable!(),
            }
            let bytes = envelope(&serde_json::to_vec(&header).unwrap(), &body);
            fs::write(&path, &bytes).unwrap();
            assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none(), "{defect}");
            assert_eq!(fs::read(&path).unwrap(), bytes, "reads must not repair {defect}");
        }
        let mut trailing = original.clone();
        trailing.push(0);
        let mut changed_body = original.clone();
        *changed_body.last_mut().unwrap() ^= 1;
        for bytes in [
            original[..original.len() - 1].to_vec(),
            original[..3].to_vec(),
            trailing,
            changed_body,
            body,
        ] {
            fs::write(&path, &bytes).unwrap();
            assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        fs::write(&path, &original).unwrap();
        assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_some());
        let other = "https://image.tmdb.org/t/p/w500/other.png";
        fs::write(cache.path_for(TMDB_PROVIDER, other), &original).unwrap();
        assert!(cache.cached_entry(TMDB_PROVIDER, other).is_none());
    }

    #[test]
    fn artwork_envelope_rejects_noncanonical_duplicate_unknown_and_oversized_headers() {
        let (root, kernel) = new_kernel();
        let cache = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());
        let body = image();
        cache
            .store(TMDB_PROVIDER, URL, &body, &policy(ProviderResponseReuse::Reusable))
            .unwrap();
        let path = cache.path_for(TMDB_PROVIDER, URL);
        let original = fs::read(&path).unwrap();
        let header = serde_json::to_string(&decode_header(&original)).unwrap();
        for changed in [
            format!(" {header}"),
            format!("{header}\n"),
            header.replacen("{", "{\"version\":1,", 1),
            header.replacen("{", "{\"unknown\":null,", 1),
            header.replacen("\"version\":1,", "", 1),
            header.replacen("\"policy\":{", "\"policy\":{\"unknown\":null,", 1),
            header.replacen("\"nanos\":0", "\"nanos\":1000000000", 1),
            header.replacen("2026-09-05T12:00:00Z", "2026-09-05T12:00:00+00:00", 1),
            "null".to_owned(),
            "{".to_owned(),
            " ".repeat(ENVELOPE_HEADER_LIMIT + 1),
        ] {
            assert_ne!(changed, header, "fixture must change the canonical header");
            let bytes = envelope(changed.as_bytes(), &body);
            fs::write(&path, &bytes).unwrap();
            assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none(), "{changed}");
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        for declared in [0_u32, ENVELOPE_HEADER_LIMIT as u32 + 1, u32::MAX] {
            fs::write(&path, declared.to_be_bytes()).unwrap();
            assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none());
        }
        let mut bytes = original.clone();
        bytes.resize(4 + ENVELOPE_HEADER_LIMIT + ARTWORK_LIMIT + 1, 0);
        fs::write(&path, bytes).unwrap();
        assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_none());
        fs::write(&path, original).unwrap();
        assert!(cache.cached_entry(TMDB_PROVIDER, URL).is_some());
    }
}
