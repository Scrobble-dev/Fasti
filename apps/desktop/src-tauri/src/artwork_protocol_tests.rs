mod protocol_tests {
    use super::*;

    const ORIGIN: &str = "http://127.0.0.1:8420";

    #[test]
    fn native_artwork_admission_caps_queued_requests_and_recovers_on_drop() {
        assert_eq!(ARTWORK_REQUESTS.available_permits(), 64);
        let mut admitted = (0..64)
            .map(|_| ARTWORK_REQUESTS.try_acquire().expect("request admitted"))
            .collect::<Vec<_>>();
        assert_eq!(ARTWORK_REQUESTS.available_permits(), 0);
        assert!(matches!(
            ARTWORK_REQUESTS.try_acquire(),
            Err(tokio::sync::TryAcquireError::NoPermits)
        ));

        drop(admitted.pop().expect("held permit"));
        assert_eq!(ARTWORK_REQUESTS.available_permits(), 1);
        let replacement = ARTWORK_REQUESTS
            .try_acquire()
            .expect("dropped request releases admission capacity");
        assert_eq!(ARTWORK_REQUESTS.available_permits(), 0);
        drop(replacement);
        drop(admitted);
        assert_eq!(ARTWORK_REQUESTS.available_permits(), 64);
        // No close/forget/global reset: RAII also releases permits on assertion failure.
    }

    fn locator_with_prefix(prefix: &str) -> String {
        format!(
            "{prefix}{}.{}.{}.{}",
            "ab".repeat(32),
            fasti_domain::WorkspaceId::new_v7(),
            fasti_domain::ProfileId::new_v7(),
            fasti_domain::RecordId::new_v7(),
        )
    }

    fn locator() -> String {
        locator_with_prefix("fasti-artwork.")
    }

    fn request(uri: &str) -> http::Request<Vec<u8>> {
        http::Request::builder().uri(uri).body(Vec::new()).unwrap()
    }

    #[test]
    fn native_artwork_request_accepts_both_asset_forms_and_optional_exact_origin() {
        for locator in [
            locator(),
            locator_with_prefix(artwork::CACHED_ARTWORK_LOCATOR_PREFIX),
        ] {
            for base in ["asset://localhost", "http://asset.localhost"] {
                let mut request = request(&format!("{base}/{locator}"));
                assert_eq!(request_locator(&request, ORIGIN), Some(locator.as_str()));
                request
                    .headers_mut()
                    .insert(http::header::ORIGIN, http::HeaderValue::from_static(ORIGIN));
                assert_eq!(request_locator(&request, ORIGIN), Some(locator.as_str()));
            }
        }
    }

    #[test]
    fn native_artwork_request_rejects_methods_bodies_and_even_empty_queries() {
        let locator = locator();
        let uri = format!("asset://localhost/{locator}");
        for method in [
            http::Method::HEAD,
            http::Method::POST,
            http::Method::PUT,
            http::Method::DELETE,
            http::Method::OPTIONS,
        ] {
            let mut request = request(&uri);
            *request.method_mut() = method;
            assert!(request_locator(&request, ORIGIN).is_none());
        }
        let mut with_body = request(&uri);
        with_body.body_mut().push(0);
        assert!(request_locator(&with_body, ORIGIN).is_none());
        for suffix in ["?", "?offline=true", "?path=/etc/passwd"] {
            let request = request(&format!("{uri}{suffix}"));
            assert!(request.uri().query().is_some());
            assert!(request_locator(&request, ORIGIN).is_none());
        }
    }

    #[test]
    fn native_artwork_request_rejects_spoofed_authority_and_nonflat_paths() {
        let locator = locator();
        for base in [
            "asset://evil.example",
            "asset://localhost.evil.example",
            "asset://user@localhost",
            "asset://localhost:80",
            "http://asset.localhost.evil.example",
            "http://user@asset.localhost",
            "http://asset.localhost:80",
            "https://asset.localhost",
            "http://localhost",
            "tauri://localhost",
        ] {
            let request = request(&format!("{base}/{locator}"));
            assert!(request_locator(&request, ORIGIN).is_none(), "{base}");
        }
        for path in [
            format!("/{locator}"),
            format!("{locator}/extra"),
            format!("{locator}%2fextra"),
            format!("{locator}%5cextra"),
            format!("{locator}%00"),
            format!("{locator}%20"),
            format!("%66{}", &locator[1..]),
            "etc/passwd".to_owned(),
        ] {
            let request = request(&format!("asset://localhost/{path}"));
            assert!(request_locator(&request, ORIGIN).is_none(), "{path}");
        }
        assert!(request_locator(&request(&format!("/{locator}")), ORIGIN).is_none());
        // Exact typed IDs and scope matching belong to artwork_selection;
        // this parser only admits a bounded, flat transport locator.
    }

    #[test]
    fn native_artwork_request_rejects_foreign_duplicate_and_binary_origins() {
        let uri = format!("asset://localhost/{}", locator());
        for value in [
            "null",
            "http://127.0.0.1:8420.evil.example",
            "http://127.0.0.1:8421",
            "https://127.0.0.1:8420",
            "http://127.0.0.1:8420/",
            "http://127.0.0.1:8420, http://evil.example",
        ] {
            let mut request = request(&uri);
            request.headers_mut().insert(
                http::header::ORIGIN,
                http::HeaderValue::from_str(value).unwrap(),
            );
            assert!(request_locator(&request, ORIGIN).is_none(), "{value}");
        }
        let mut duplicate = request(&uri);
        for _ in 0..2 {
            duplicate
                .headers_mut()
                .append(http::header::ORIGIN, http::HeaderValue::from_static(ORIGIN));
        }
        assert!(request_locator(&duplicate, ORIGIN).is_none());
        let mut binary = request(&uri);
        binary.headers_mut().insert(
            http::header::ORIGIN,
            http::HeaderValue::from_bytes(&[0xff]).unwrap(),
        );
        assert!(request_locator(&binary, ORIGIN).is_none());
    }

    #[test]
    fn native_artwork_request_enforces_locator_and_header_bounds() {
        let prefix = "fasti-artwork.";
        for (length, accepted) in [(256, true), (257, false), (400, false)] {
            let locator = format!("{prefix}{}", "a".repeat(length - prefix.len()));
            let request = request(&format!("asset://localhost/{locator}"));
            assert_eq!(request_locator(&request, ORIGIN).is_some(), accepted);
        }
        let uri = format!("asset://localhost/{}", locator());
        let mut counted = request(&uri);
        for index in 0..32 {
            counted.headers_mut().insert(
                http::header::HeaderName::from_bytes(format!("x-{index}").as_bytes()).unwrap(),
                http::HeaderValue::from_static("a"),
            );
        }
        assert!(request_locator(&counted, ORIGIN).is_some());
        counted
            .headers_mut()
            .insert("x-extra", http::HeaderValue::from_static("a"));
        assert!(request_locator(&counted, ORIGIN).is_none());
        for (value_length, accepted) in [(8191, true), (8192, false)] {
            let mut request = request(&uri);
            request.headers_mut().insert(
                "x",
                http::HeaderValue::from_str(&"a".repeat(value_length)).unwrap(),
            );
            assert_eq!(request_locator(&request, ORIGIN).is_some(), accepted);
        }
    }

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    #[test]
    fn native_artwork_response_serves_only_bounded_images_without_webview_caching() {
        // Synthetic image headers exercise the existing bounded classifier,
        // not browser decoding or a live provider response.
        for size in [24, 2_000_000] {
            let mut bytes = png_header(500, 750);
            bytes.resize(size, 0);
            let delivered = response(Some(bytes.clone()), Some(ORIGIN));
            assert_eq!(delivered.status(), http::StatusCode::OK);
            assert_eq!(delivered.body(), &bytes);
            assert_eq!(delivered.headers()[http::header::CONTENT_TYPE], "image/png");
            assert_eq!(delivered.headers()[http::header::CACHE_CONTROL], "no-store");
            assert_eq!(delivered.headers()["x-content-type-options"], "nosniff");
            assert_eq!(
                delivered.headers()[http::header::ACCESS_CONTROL_ALLOW_ORIGIN],
                ORIGIN,
            );
            assert!(!delivered
                .headers()
                .contains_key(http::header::ACCESS_CONTROL_ALLOW_CREDENTIALS));
        }
    }

    #[test]
    fn native_artwork_errors_are_empty_non_disclosing_and_never_cacheable() {
        let mut oversized = png_header(500, 750);
        oversized.resize(2_000_001, 0);
        for bytes in [
            None,
            Some(Vec::new()),
            Some(b"<svg><script>alert(1)</script></svg>".to_vec()),
            Some(b"/private/cache/provider-artwork: https://provider.example/key".to_vec()),
            Some(png_header(0, 1)),
            Some(png_header(4097, 1)),
            Some(png_header(4096, 4096)),
            Some(oversized),
        ] {
            for origin in [None, Some(ORIGIN)] {
                let delivered = response(bytes.clone(), origin);
                assert_eq!(delivered.status(), http::StatusCode::NOT_FOUND);
                assert!(delivered.body().is_empty());
                assert_eq!(delivered.headers()[http::header::CACHE_CONTROL], "no-store");
                assert_eq!(delivered.headers()["x-content-type-options"], "nosniff");
                assert!(!delivered.headers().contains_key(http::header::CONTENT_TYPE));
                assert_eq!(
                    delivered
                        .headers()
                        .get(http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
                        .map(|value| value.to_str().unwrap()),
                    origin,
                );
                assert!(!delivered.headers().contains_key(http::header::LOCATION));
                assert!(!delivered.headers().contains_key(http::header::SET_COOKIE));
            }
        }
    }
}
