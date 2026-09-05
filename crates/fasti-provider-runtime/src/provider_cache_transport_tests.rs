mod cache_transport_tests {
    use super::*;
    use fasti_application::ProviderResponseReuse;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // This deliberately tests send_json's HTTP boundary, not governed provider
    // DNS, TLS, credential access, or provider availability.
    async fn fixture(
        headers: String,
        body: Vec<u8>,
        body_gate: Option<tokio::sync::oneshot::Receiver<()>>,
    ) -> (reqwest::RequestBuilder, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(5), async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    assert!(request.len() < 8192, "bounded fixture request");
                    request.push(socket.read_u8().await.unwrap());
                }
                let request = String::from_utf8(request).unwrap();
                assert!(!request.to_ascii_lowercase().contains("authorization:"));
                socket.write_all(headers.as_bytes()).await.unwrap();
                if let Some(gate) = body_gate {
                    gate.await.unwrap();
                }
                // An early rejection is allowed to close the socket.
                let _ = socket.write_all(&body).await;
                let _ = socket.shutdown().await;
            })
            .await
            .expect("bounded loopback fixture");
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        (client.get(format!("http://{address}/fixture")), server)
    }

    async fn response(body: &[u8], cache_control: &str) -> ProviderJsonResponse {
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: {cache_control}\r\nX-Fixture-Private: never-public\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let (request, server) = fixture(headers, body.to_vec(), None).await;
        let result = send_json(request, TMDB_SPEC).await.unwrap();
        server.await.unwrap();
        assert_eq!(result.body, body);
        result
    }

    #[tokio::test]
    async fn transport_observes_headers_before_waiting_for_body() {
        let body = b"{\"ok\":true}";
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: max-age=60, must-revalidate\r\nAge: 10\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let (release, gate) = tokio::sync::oneshot::channel();
        let (request, server) = fixture(headers, body.to_vec(), Some(gate)).await;
        let before = chrono::Utc::now();
        let pending = send_json(request, TMDB_SPEC);
        tokio::pin!(pending);
        // Poll the actual reader while the server withholds the body. The
        // observation must not move forward when that body is later released.
        tokio::select! {
            result = &mut pending => panic!("body gate unexpectedly completed: {}", result.is_ok()),
            () = tokio::time::sleep(Duration::from_millis(250)) => {}
        }
        let body_released_at = chrono::Utc::now();
        release.send(()).unwrap();
        let result = pending.await.unwrap();
        server.await.unwrap();
        assert_eq!(result.body, body);
        let policy = result.cache_policy;
        assert_eq!(policy.reuse(), ProviderResponseReuse::ValidateWhenStale);
        assert!(before <= policy.received_at());
        assert!(policy.received_at() < body_released_at);
        let (fresh, stale) = policy
            .deadlines(Duration::from_secs(3600), Duration::from_secs(7200))
            .unwrap();
        assert_eq!(fresh, stale);
        assert!(fresh <= policy.received_at() + chrono::Duration::seconds(50));
        assert!(fresh > policy.received_at());
    }

    #[tokio::test]
    async fn transport_policy_survives_nonempty_empty_and_filtered_search_pages() {
        let fixtures: &[(&[u8], bool, usize, Option<u32>)] = &[
            (br#"{"page":1,"total_pages":2,"results":[{"id":42,"media_type":"movie","title":"Film","adult":false}]}"#, true, 1, Some(2)),
            (br#"{"page":1,"total_pages":0,"results":[]}"#, true, 0, None),
            (br#"{"page":1,"total_pages":2,"results":[{"id":42,"media_type":"movie","title":"Filtered","adult":true}]}"#, true, 0, Some(2)),
            (br#"{"totalItems":1,"items":[{"id":"book-1","volumeInfo":{"title":"Book"}}]}"#, false, 1, None),
            (br#"{"totalItems":0,"items":[]}"#, false, 0, None),
            (br#"{"totalItems":100,"items":[{"id":"book-1"}]}"#, false, 0, Some(2)),
        ];
        for &(body, tmdb, count, next) in fixtures {
            let response = response(body, "private, no-cache").await;
            let page = if tmdb {
                parse_tmdb_candidates(&response.body, 1)
            } else {
                parse_google_candidates(&response.body, 1)
            }
            .unwrap()
            .with_response_cache_policy(response.cache_policy);
            assert_eq!(page.response_cache_policy(), Some(&response.cache_policy));
            assert_eq!(page.candidates.len(), count);
            assert_eq!(page.next_page, next);
            assert_eq!(page.evidence_digest, provider_evidence_digest(body));
            for candidate in page.candidates {
                assert_eq!(
                    candidate.response_cache_policy(),
                    Some(&response.cache_policy)
                );
                assert_public_candidate(&candidate);
            }
        }
    }

    fn assert_public_candidate(candidate: &ProviderCandidate) {
        let public = serde_json::to_value(candidate).unwrap();
        let object = public.as_object().unwrap();
        let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
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
        assert!(!public.to_string().contains("never-public"));
    }

    #[tokio::test]
    async fn transport_policy_survives_both_detail_parsers_without_public_serialization() {
        let body = br#"{"id":42,"title":"Film","adult":false}"#;
        let response = response(body, "no-store").await;
        let candidate = tmdb_candidate(
            serde_json::from_slice(&response.body).unwrap(),
            Some("movie"),
            provider_evidence_digest(&response.body),
        )
        .unwrap();
        let candidate = verify_selected_candidate(candidate, "42", "movie")
            .unwrap()
            .with_response_cache_policy(response.cache_policy);
        assert_eq!(
            candidate.response_cache_policy(),
            Some(&response.cache_policy)
        );
        assert_eq!(
            response.cache_policy.reuse(),
            ProviderResponseReuse::NoStore
        );
        assert_public_candidate(&candidate);

        let body = br#"{"id":"book-1","volumeInfo":{"title":"Book"}}"#;
        let response = self::response(body, "max-age=30").await;
        let candidate = google_candidate(
            serde_json::from_slice(&response.body).unwrap(),
            provider_evidence_digest(&response.body),
        )
        .unwrap();
        let candidate = verify_selected_candidate(candidate, "book-1", "book")
            .unwrap()
            .with_response_cache_policy(response.cache_policy);
        assert_eq!(
            candidate.response_cache_policy(),
            Some(&response.cache_policy)
        );
        assert_eq!(
            response.cache_policy.reuse(),
            ProviderResponseReuse::Reusable
        );
        assert_public_candidate(&candidate);
    }

    #[tokio::test]
    async fn transport_keeps_status_and_content_type_failure_classification() {
        for (status, spec, expected) in [
            (401, TMDB_SPEC, ProblemCode::ProviderCredentialInvalid),
            (403, TMDB_SPEC, ProblemCode::ProviderCredentialInvalid),
            (
                400,
                GOOGLE_BOOKS_SPEC,
                ProblemCode::ProviderCredentialInvalid,
            ),
            (429, TMDB_SPEC, ProblemCode::ProviderRateLimited),
            (503, TMDB_SPEC, ProblemCode::ProviderUnavailable),
            (404, TMDB_SPEC, ProblemCode::ProviderResponseInvalid),
            (200, TMDB_SPEC, ProblemCode::ProviderResponseInvalid),
        ] {
            let headers = format!(
                "HTTP/1.1 {status} Fixture\r\nContent-Type: text/plain\r\nContent-Length: 0\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
            );
            let (request, server) = fixture(headers, Vec::new(), None).await;
            let error = send_json(request, spec)
                .await
                .err()
                .expect("rejected response");
            server.await.unwrap();
            assert_eq!(error.problem_code(), expected, "HTTP {status}");
        }
    }

    #[tokio::test]
    async fn transport_keeps_exact_body_limit_and_rejects_chunked_overflow_and_truncation() {
        let body = vec![b' '; RESPONSE_LIMIT];
        assert_eq!(
            response(&body, "max-age=30").await.body.len(),
            RESPONSE_LIMIT
        );
        let overflow = vec![b' '; RESPONSE_LIMIT + 1];
        let mut chunked = format!("{:x}\r\n", overflow.len()).into_bytes();
        chunked.extend_from_slice(&overflow);
        chunked.extend_from_slice(b"\r\n0\r\n\r\n");
        for (framing, body) in [
            (format!("Content-Length: {}", RESPONSE_LIMIT + 1), overflow),
            ("Transfer-Encoding: chunked".to_owned(), chunked),
            ("Content-Length: 10".to_owned(), b"{}".to_vec()),
        ] {
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{framing}\r\nCache-Control: max-age=30\r\nConnection: close\r\n\r\n"
            );
            let (request, server) = fixture(headers, body, None).await;
            let error = send_json(request, TMDB_SPEC)
                .await
                .err()
                .expect("bounded body rejection");
            server.await.unwrap();
            assert_eq!(error.problem_code(), ProblemCode::ProviderResponseInvalid);
        }
    }
}
