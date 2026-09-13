mod kitsu_continuation_tests {
    use super::super::kitsu::Continuation;
    use std::collections::BTreeSet;

    #[test]
    fn kitsu_continuation_decodes_both_turns_and_single_source_tails() {
        for (token, kind, source_page) in [
            (1, "anime", 1),
            (2, "manga", 1),
            (3, "anime", 1),
            (4, "manga", 1),
            (5, "anime", 2),
            (6, "manga", 2),
            (7, "anime", 2),
            (8, "manga", 2),
            (397, "anime", 100),
            (400, "manga", 100),
        ] {
            let continuation = Continuation::decode(token).expect("valid continuation");
            assert_eq!(continuation.kind(), kind, "token {token}");
            assert_eq!(continuation.source_page(), source_page, "token {token}");
        }
        assert!(Continuation::decode(0).is_err());
    }

    #[test]
    fn kitsu_continuation_advances_source_links_and_exhaustion_strictly_forward() {
        for (token, has_next, expected) in [
            (1, true, Some(2)),
            (1, false, Some(4)),
            (2, true, Some(5)),
            (2, false, Some(7)),
            (3, true, Some(7)),
            (3, false, None),
            (4, true, Some(8)),
            (4, false, None),
            (397, true, Some(398)),
            (397, false, Some(400)),
            (398, true, Some(401)),
            (398, false, Some(403)),
            (399, true, Some(403)),
            (399, false, None),
            (400, true, Some(404)),
            (400, false, None),
        ] {
            let next = Continuation::decode(token)
                .expect("valid continuation")
                .advance(has_next)
                .expect("valid transition");
            assert_eq!(next, expected, "token {token}, has_next {has_next}");
            if let Some(next) = next {
                assert!(next > token);
                assert!(Continuation::decode(next).is_ok());
            }
        }
    }

    #[test]
    fn kitsu_continuation_visits_each_source_page_once_and_drains_the_longer_tail() {
        // A zero-length source still has one real empty HTTP response to observe.
        for (anime_pages, manga_pages, expected_tokens) in [
            (0, 0, vec![1, 4]),
            (0, 3, vec![1, 4, 8, 12]),
            (3, 0, vec![1, 2, 7, 11]),
            (1, 3, vec![1, 4, 8, 12]),
            (3, 1, vec![1, 2, 7, 11]),
            (2, 2, vec![1, 2, 5, 8]),
            (2, 3, vec![1, 2, 5, 8, 12]),
            (3, 2, vec![1, 2, 5, 6, 11]),
        ] {
            let mut token = Some(1);
            let mut tokens = Vec::new();
            let mut visited = BTreeSet::new();
            while let Some(current) = token {
                assert!(tokens.len() < 8, "continuation must terminate");
                let continuation = Continuation::decode(current).expect("valid continuation");
                let source_page = continuation.source_page();
                let kind = continuation.kind();
                assert!(visited.insert((kind, source_page)), "source page repeated");
                let pages = if kind == "anime" {
                    anime_pages
                } else {
                    manga_pages
                };
                assert!(source_page <= pages.max(1), "source tail was overrun");
                tokens.push(current);
                token = continuation.advance(source_page < pages).expect("advance");
                if let Some(next) = token {
                    assert!(next > current);
                }
            }
            assert_eq!(
                tokens, expected_tokens,
                "anime {anime_pages}, manga {manga_pages}"
            );
            let expected = (1..=anime_pages.max(1))
                .map(|page| ("anime", page))
                .chain((1..=manga_pages.max(1)).map(|page| ("manga", page)))
                .collect::<BTreeSet<_>>();
            assert_eq!(visited, expected, "no source tail may be skipped");
        }
    }

    #[test]
    fn kitsu_continuation_preserves_the_full_checked_source_offset_range() {
        let last_source_page = u32::MAX / 10 + 1;
        assert_eq!(last_source_page, 429_496_730);
        let last_base = (last_source_page - 1) * 4;
        assert_eq!(last_base, 1_717_986_916);
        for mode in 1..=4 {
            let token = last_base + mode;
            let continuation = Continuation::decode(token).expect("last addressable page");
            assert_eq!(continuation.source_page(), last_source_page);
            assert_eq!(
                (continuation.source_page() - 1).checked_mul(10),
                Some(4_294_967_290)
            );
            assert!(
                continuation.advance(true).is_err(),
                "token {token} must reject an unaddressable reported source next"
            );
        }
        assert_eq!(
            Continuation::decode(last_base + 1)
                .unwrap()
                .advance(false)
                .unwrap(),
            Some(last_base + 4),
            "Anime exhaustion can still visit Manga at the same final source page"
        );
        assert!(Continuation::decode(last_base + 2)
            .unwrap()
            .advance(false)
            .is_err());
        for mode in [3, 4] {
            assert_eq!(
                Continuation::decode(last_base + mode)
                    .unwrap()
                    .advance(false)
                    .unwrap(),
                None
            );
        }
        for token in [last_base + 5, last_base + 8, u32::MAX - 1, u32::MAX] {
            assert!(
                Continuation::decode(token).is_err(),
                "invalid token {token}"
            );
        }
        for (token, expected) in [
            (last_base - 3, last_base - 2),
            (last_base - 2, last_base + 1),
            (last_base - 1, last_base + 3),
            (last_base, last_base + 4),
        ] {
            assert_eq!(
                Continuation::decode(token).unwrap().advance(true).unwrap(),
                Some(expected),
                "the last valid source next remains reachable"
            );
        }
    }
}
