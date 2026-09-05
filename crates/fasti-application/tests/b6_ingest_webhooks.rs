#![cfg(feature = "conformance-fixture")]

//! Converter and in-memory conformance tests for Plex, Jellyfin/Emby, and Linux MPRIS-shaped input.

use fasti_application::{
    conformance::B1ConformanceFixture,
    ingest::{
        JellyfinWebhookPayload, MprisMediaEvent, PlexAccount, PlexGuidItem, PlexMetadata,
        PlexWebhookPayload,
    },
    AcceptObservationOutcome, ObservationAcceptancePort,
};
use fasti_domain::{ClaimedTrust, Grain, ObservedAt, RequestCorrelationId};

fn enroll(fixture: &B1ConformanceFixture) -> fasti_application::conformance::FixtureEnrollment {
    let init = fixture
        .initialize_node(RequestCorrelationId::new_v7())
        .expect("init")
        .into_inner();
    fixture
        .enroll_first_client(RequestCorrelationId::new_v7(), &init)
        .expect("enroll")
        .into_inner()
}

fn sample_observed_at(instant: &str) -> ObservedAt {
    ObservedAt::parse(instant, ClaimedTrust::DeviceObserved).expect("valid observed_at")
}

#[test]
fn plex_webhook_movie_scrobble_commits_and_replays() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let payload = PlexWebhookPayload {
        event: "media.scrobble".to_owned(),
        user: true,
        owner: true,
        account: Some(PlexAccount {
            id: 101,
            title: "alice".to_owned(),
        }),
        metadata: Some(PlexMetadata {
            rating_key: "plex-12345".to_owned(),
            media_type: "movie".to_owned(),
            title: "Spirited Away".to_owned(),
            grandparent_title: None,
            parent_index: None,
            index: None,
            year: Some(2001),
            duration: Some(7500000),
            view_offset: Some(7500000),
            guid: Some("plex://movie/123".to_owned()),
            guids: vec![
                PlexGuidItem {
                    id: "tmdb://129".to_owned(),
                },
                PlexGuidItem {
                    id: "imdb://tt0245429".to_owned(),
                },
            ],
        }),
    };

    let cmd = payload
        .to_observation_command(access, sample_observed_at("2026-08-25T11:00:00Z"))
        .expect("generates valid command");

    assert_eq!(cmd.target_grain(), Some(Grain::Film));
    assert_eq!(cmd.identity_clues().len(), 2);

    // First dispatch -> Committed
    let outcome1 = fixture
        .authorize_and_accept(cmd.clone())
        .expect("accepts observation");
    assert!(
        matches!(outcome1, AcceptObservationOutcome::Committed(_)),
        "first observation should be committed"
    );

    // Second dispatch with exact same webhook -> Replayed with equal receipt
    let outcome2 = fixture
        .authorize_and_accept(cmd)
        .expect("replays observation");
    match (outcome1, outcome2) {
        (
            AcceptObservationOutcome::Committed(receipt1),
            AcceptObservationOutcome::Replayed(receipt2),
        ) => {
            assert_eq!(receipt1, receipt2, "replayed receipt must be identical");
        }
        _ => panic!("expected committed then replayed outcome"),
    }
}

#[test]
fn plex_webhook_pause_event_is_ignored() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let payload = PlexWebhookPayload {
        event: "media.pause".to_owned(),
        user: true,
        owner: true,
        account: Some(PlexAccount {
            id: 101,
            title: "alice".to_owned(),
        }),
        metadata: Some(PlexMetadata {
            rating_key: "plex-12345".to_owned(),
            media_type: "movie".to_owned(),
            title: "Spirited Away".to_owned(),
            grandparent_title: None,
            parent_index: None,
            index: None,
            year: Some(2001),
            duration: Some(7500000),
            view_offset: Some(3000000),
            guid: None,
            guids: vec![],
        }),
    };

    let cmd = payload.to_observation_command(access, sample_observed_at("2026-08-25T11:05:00Z"));
    assert!(
        cmd.is_none(),
        "pause event must not generate observation command"
    );
}

#[test]
fn jellyfin_webhook_playback_stop_commits_and_preserves_identifiers() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let payload = JellyfinWebhookPayload {
        notification_type: "PlaybackStop".to_owned(),
        item_type: "Episode".to_owned(),
        item_id: "jf-ep-42".to_owned(),
        name: "Ballad of Fallen Angels".to_owned(),
        series_name: Some("Cowboy Bebop".to_owned()),
        season_number: Some(1),
        episode_number: Some(5),
        year: Some(1998),
        provider_tmdb: Some("2490".to_owned()),
        provider_imdb: Some("tt0618968".to_owned()),
        provider_tvdb: Some("76142".to_owned()),
        played_to_completion: true,
        user_id: Some("user-jellyfin-1".to_owned()),
        playback_position_ticks: Some(12_600_000_000),
    };

    let cmd = payload
        .to_observation_command(access, sample_observed_at("2026-08-25T11:30:00Z"))
        .expect("generates valid command");

    assert_eq!(cmd.target_grain(), Some(Grain::Episode));
    assert_eq!(
        cmd.identity_clues().len(),
        2,
        "IMDb and TVDB remain; a series-level TMDB ID must not become an Episode coordinate"
    );
    assert!(cmd
        .identity_clues()
        .iter()
        .all(|claim| claim.namespace() != "tmdb.tv"));

    let outcome = fixture.authorize_and_accept(cmd).expect("accepts");
    assert!(matches!(outcome, AcceptObservationOutcome::Committed(_)));
}

#[test]
fn mpris_desktop_track_completion_commits_and_deduplicates() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let event = MprisMediaEvent {
        player_identity: "VLC".to_owned(),
        track_id: "file:///music/daft_punk/discovery/one_more_time.flac".to_owned(),
        title: "One More Time".to_owned(),
        artist: Some("Daft Punk".to_owned()),
        album: Some("Discovery".to_owned()),
        duration_micros: Some(320000000),
        position_micros: Some(320000000),
        is_completed: true,
    };

    let cmd = event.to_observation_command(access, sample_observed_at("2026-08-25T12:00:00Z"));
    assert_eq!(cmd.target_grain(), Some(Grain::Track));

    let outcome1 = fixture.authorize_and_accept(cmd.clone()).expect("accepts");
    assert!(matches!(outcome1, AcceptObservationOutcome::Committed(_)));

    let outcome2 = fixture.authorize_and_accept(cmd).expect("replays");
    assert!(matches!(outcome2, AcceptObservationOutcome::Replayed(_)));
}

#[test]
fn plex_webhook_rewatch_at_a_different_offset_is_a_distinct_commit() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let base = PlexWebhookPayload {
        event: "media.scrobble".to_owned(),
        user: true,
        owner: true,
        account: Some(PlexAccount {
            id: 101,
            title: "alice".to_owned(),
        }),
        metadata: Some(PlexMetadata {
            rating_key: "plex-99999".to_owned(),
            media_type: "movie".to_owned(),
            title: "Spirited Away".to_owned(),
            grandparent_title: None,
            parent_index: None,
            index: None,
            year: Some(2001),
            duration: Some(7500000),
            view_offset: Some(7500000),
            guid: None,
            guids: vec![],
        }),
    };
    let first_watch = base.clone();
    let mut second_watch = base;
    second_watch.metadata.as_mut().unwrap().view_offset = Some(7480000);

    let first_cmd = first_watch
        .to_observation_command(access, sample_observed_at("2026-08-25T11:00:00Z"))
        .expect("generates valid command");
    let second_cmd = second_watch
        .to_observation_command(access, sample_observed_at("2026-08-26T11:00:00Z"))
        .expect("generates valid command");

    assert_ne!(
        first_cmd.operation_id(),
        second_cmd.operation_id(),
        "a rewatch must not be silently deduplicated against the prior play"
    );

    let outcome1 = fixture.authorize_and_accept(first_cmd).expect("accepts");
    assert!(matches!(outcome1, AcceptObservationOutcome::Committed(_)));
    let outcome2 = fixture.authorize_and_accept(second_cmd).expect("accepts");
    assert!(
        matches!(outcome2, AcceptObservationOutcome::Committed(_)),
        "the rewatch must commit as its own observation, not replay the first watch's receipt"
    );
}

#[test]
fn jellyfin_webhook_rewatch_at_a_different_position_is_a_distinct_commit() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let base = JellyfinWebhookPayload {
        notification_type: "PlaybackStop".to_owned(),
        item_type: "Movie".to_owned(),
        item_id: "jf-movie-1".to_owned(),
        name: "Spirited Away".to_owned(),
        series_name: None,
        season_number: None,
        episode_number: None,
        year: Some(2001),
        provider_tmdb: None,
        provider_imdb: None,
        provider_tvdb: None,
        played_to_completion: true,
        user_id: Some("user-jellyfin-1".to_owned()),
        playback_position_ticks: Some(75_000_000_000),
    };
    let first_watch = base.clone();
    let second_watch = JellyfinWebhookPayload {
        playback_position_ticks: Some(74_800_000_000),
        ..base
    };

    let first_cmd = first_watch
        .to_observation_command(access, sample_observed_at("2026-08-25T11:00:00Z"))
        .expect("generates valid command");
    let second_cmd = second_watch
        .to_observation_command(access, sample_observed_at("2026-08-26T11:00:00Z"))
        .expect("generates valid command");

    assert_ne!(
        first_cmd.operation_id(),
        second_cmd.operation_id(),
        "a rewatch must not be silently deduplicated against the prior play"
    );

    let outcome1 = fixture.authorize_and_accept(first_cmd).expect("accepts");
    assert!(matches!(outcome1, AcceptObservationOutcome::Committed(_)));
    let outcome2 = fixture.authorize_and_accept(second_cmd).expect("accepts");
    assert!(
        matches!(outcome2, AcceptObservationOutcome::Committed(_)),
        "the rewatch must commit as its own observation, not replay the first watch's receipt"
    );
}

#[test]
fn plex_administrative_event_without_metadata_deserializes_and_is_ignored() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();
    // A real Plex administrative/library-maintenance payload: no `Metadata`
    // object at all, since it isn't about a specific media item.
    let raw = r#"{
        "event": "admin.database.backup",
        "user": true,
        "owner": true,
        "Account": { "id": 101, "title": "alice" }
    }"#;

    let payload: PlexWebhookPayload =
        serde_json::from_str(raw).expect("administrative payload without Metadata deserializes");
    assert!(payload.metadata.is_none());

    let cmd = payload.to_observation_command(access, sample_observed_at("2026-08-25T11:00:00Z"));
    assert!(
        cmd.is_none(),
        "an administrative event has no media item to record an observation about"
    );
}

#[test]
fn plex_webhook_without_an_account_is_rejected_rather_than_using_a_synthetic_identity() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();
    let payload = PlexWebhookPayload {
        event: "media.scrobble".to_owned(),
        user: true,
        owner: true,
        account: None,
        metadata: Some(PlexMetadata {
            rating_key: "plex-no-account".to_owned(),
            media_type: "movie".to_owned(),
            title: "Spirited Away".to_owned(),
            grandparent_title: None,
            parent_index: None,
            index: None,
            year: Some(2001),
            duration: Some(7500000),
            view_offset: Some(7500000),
            guid: None,
            guids: vec![],
        }),
    };

    let cmd = payload.to_observation_command(access, sample_observed_at("2026-08-25T11:00:00Z"));
    assert!(
        cmd.is_none(),
        "a payload with no account must not synthesize a shared source identity"
    );
}

#[test]
fn jellyfin_webhook_without_a_user_id_is_rejected_rather_than_using_a_synthetic_identity() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();
    let payload = JellyfinWebhookPayload {
        notification_type: "PlaybackStop".to_owned(),
        item_type: "Movie".to_owned(),
        item_id: "jf-movie-no-user".to_owned(),
        name: "Spirited Away".to_owned(),
        series_name: None,
        season_number: None,
        episode_number: None,
        year: Some(2001),
        provider_tmdb: None,
        provider_imdb: None,
        provider_tvdb: None,
        played_to_completion: true,
        user_id: None,
        playback_position_ticks: Some(75_000_000_000),
    };

    let cmd = payload.to_observation_command(access, sample_observed_at("2026-08-25T11:00:00Z"));
    assert!(
        cmd.is_none(),
        "a payload with no user id must not synthesize a shared source identity"
    );
}

#[test]
fn plex_completed_rewatch_at_the_same_terminal_offset_on_a_different_day_is_a_distinct_commit() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    // A completed watch and a completed rewatch of the same movie both end
    // at (near) the same terminal view_offset -- offset alone cannot
    // distinguish them.
    let base = PlexWebhookPayload {
        event: "media.scrobble".to_owned(),
        user: true,
        owner: true,
        account: Some(PlexAccount {
            id: 101,
            title: "alice".to_owned(),
        }),
        metadata: Some(PlexMetadata {
            rating_key: "plex-completed-rewatch".to_owned(),
            media_type: "movie".to_owned(),
            title: "Spirited Away".to_owned(),
            grandparent_title: None,
            parent_index: None,
            index: None,
            year: Some(2001),
            duration: Some(7500000),
            view_offset: Some(7500000),
            guid: None,
            guids: vec![],
        }),
    };
    let first_watch = base.clone();
    let second_watch = base;

    let first_cmd = first_watch
        .to_observation_command(access, sample_observed_at("2026-08-25T23:50:00Z"))
        .expect("generates valid command");
    let second_cmd = second_watch
        .to_observation_command(access, sample_observed_at("2026-09-24T23:50:00Z"))
        .expect("generates valid command");

    assert_ne!(
        first_cmd.operation_id(),
        second_cmd.operation_id(),
        "a completed rewatch at the same terminal offset must not collide just because \
         both watches ended at the same position"
    );

    let outcome1 = fixture.authorize_and_accept(first_cmd).expect("accepts");
    assert!(matches!(outcome1, AcceptObservationOutcome::Committed(_)));
    let outcome2 = fixture.authorize_and_accept(second_cmd).expect("accepts");
    assert!(
        matches!(outcome2, AcceptObservationOutcome::Committed(_)),
        "the completed rewatch must commit as its own observation"
    );
}

#[test]
fn plex_completed_rewatch_at_the_same_terminal_offset_the_same_day_is_a_distinct_commit() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    // Two completed watches on the same calendar day, both ending at the
    // same terminal offset: a day-level discriminator alone would collide
    // these. The full observed_at instant must not.
    let base = PlexWebhookPayload {
        event: "media.scrobble".to_owned(),
        user: true,
        owner: true,
        account: Some(PlexAccount {
            id: 101,
            title: "alice".to_owned(),
        }),
        metadata: Some(PlexMetadata {
            rating_key: "plex-same-day-rewatch".to_owned(),
            media_type: "movie".to_owned(),
            title: "Spirited Away".to_owned(),
            grandparent_title: None,
            parent_index: None,
            index: None,
            year: Some(2001),
            duration: Some(7500000),
            view_offset: Some(7500000),
            guid: None,
            guids: vec![],
        }),
    };
    let first_watch = base.clone();
    let second_watch = base;

    let first_cmd = first_watch
        .to_observation_command(access, sample_observed_at("2026-08-25T10:00:00Z"))
        .expect("generates valid command");
    let second_cmd = second_watch
        .to_observation_command(access, sample_observed_at("2026-08-25T22:00:00Z"))
        .expect("generates valid command");

    assert_ne!(
        first_cmd.operation_id(),
        second_cmd.operation_id(),
        "two same-day completed occurrences at the same terminal offset must not collide"
    );

    let outcome1 = fixture.authorize_and_accept(first_cmd).expect("accepts");
    assert!(matches!(outcome1, AcceptObservationOutcome::Committed(_)));
    let outcome2 = fixture.authorize_and_accept(second_cmd).expect("accepts");
    assert!(
        matches!(outcome2, AcceptObservationOutcome::Committed(_)),
        "the second same-day occurrence must commit as its own observation"
    );
}

#[test]
fn plex_webhook_redelivered_with_the_same_observed_at_replays_rather_than_duplicating() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    // A genuine redelivery of the exact same webhook: identical payload,
    // and the caller supplies the same observed_at both times (the
    // documented contract for what "retry" means for this converter).
    let payload = PlexWebhookPayload {
        event: "media.scrobble".to_owned(),
        user: true,
        owner: true,
        account: Some(PlexAccount {
            id: 101,
            title: "alice".to_owned(),
        }),
        metadata: Some(PlexMetadata {
            rating_key: "plex-redelivery".to_owned(),
            media_type: "movie".to_owned(),
            title: "Spirited Away".to_owned(),
            grandparent_title: None,
            parent_index: None,
            index: None,
            year: Some(2001),
            duration: Some(7500000),
            view_offset: Some(7500000),
            guid: None,
            guids: vec![],
        }),
    };

    let first_attempt = payload
        .to_observation_command(access, sample_observed_at("2026-08-25T10:00:00Z"))
        .expect("generates valid command");
    let redelivery = payload
        .to_observation_command(access, sample_observed_at("2026-08-25T10:00:00Z"))
        .expect("generates valid command");

    assert_eq!(
        first_attempt.operation_id(),
        redelivery.operation_id(),
        "a redelivery with the same observed_at must derive the same operation id"
    );

    let outcome1 = fixture
        .authorize_and_accept(first_attempt)
        .expect("accepts");
    assert!(matches!(outcome1, AcceptObservationOutcome::Committed(_)));
    let outcome2 = fixture.authorize_and_accept(redelivery).expect("replays");
    assert!(
        matches!(outcome2, AcceptObservationOutcome::Replayed(_)),
        "the redelivery must replay the first attempt's receipt, not duplicate it"
    );
}

#[test]
fn jellyfin_completed_rewatch_at_the_same_terminal_position_the_same_day_is_a_distinct_commit() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let base = JellyfinWebhookPayload {
        notification_type: "PlaybackStop".to_owned(),
        item_type: "Movie".to_owned(),
        item_id: "jf-movie-same-day".to_owned(),
        name: "Spirited Away".to_owned(),
        series_name: None,
        season_number: None,
        episode_number: None,
        year: Some(2001),
        provider_tmdb: None,
        provider_imdb: None,
        provider_tvdb: None,
        played_to_completion: true,
        user_id: Some("user-jellyfin-1".to_owned()),
        playback_position_ticks: Some(75_000_000_000),
    };
    let first_watch = base.clone();
    let second_watch = base;

    let first_cmd = first_watch
        .to_observation_command(access, sample_observed_at("2026-08-25T10:00:00Z"))
        .expect("generates valid command");
    let second_cmd = second_watch
        .to_observation_command(access, sample_observed_at("2026-08-25T22:00:00Z"))
        .expect("generates valid command");

    assert_ne!(
        first_cmd.operation_id(),
        second_cmd.operation_id(),
        "two same-day completed occurrences at the same terminal position must not collide"
    );

    let outcome1 = fixture.authorize_and_accept(first_cmd).expect("accepts");
    assert!(matches!(outcome1, AcceptObservationOutcome::Committed(_)));
    let outcome2 = fixture.authorize_and_accept(second_cmd).expect("accepts");
    assert!(
        matches!(outcome2, AcceptObservationOutcome::Committed(_)),
        "the second same-day occurrence must commit as its own observation"
    );
}

#[test]
fn jellyfin_webhook_redelivered_with_the_same_observed_at_replays_rather_than_duplicating() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let payload = JellyfinWebhookPayload {
        notification_type: "PlaybackStop".to_owned(),
        item_type: "Movie".to_owned(),
        item_id: "jf-movie-redelivery".to_owned(),
        name: "Spirited Away".to_owned(),
        series_name: None,
        season_number: None,
        episode_number: None,
        year: Some(2001),
        provider_tmdb: None,
        provider_imdb: None,
        provider_tvdb: None,
        played_to_completion: true,
        user_id: Some("user-jellyfin-1".to_owned()),
        playback_position_ticks: Some(75_000_000_000),
    };

    let first_attempt = payload
        .to_observation_command(access, sample_observed_at("2026-08-25T10:00:00Z"))
        .expect("generates valid command");
    let redelivery = payload
        .to_observation_command(access, sample_observed_at("2026-08-25T10:00:00Z"))
        .expect("generates valid command");

    assert_eq!(
        first_attempt.operation_id(),
        redelivery.operation_id(),
        "a redelivery with the same observed_at must derive the same operation id"
    );

    let outcome1 = fixture
        .authorize_and_accept(first_attempt)
        .expect("accepts");
    assert!(matches!(outcome1, AcceptObservationOutcome::Committed(_)));
    let outcome2 = fixture.authorize_and_accept(redelivery).expect("replays");
    assert!(
        matches!(outcome2, AcceptObservationOutcome::Replayed(_)),
        "the redelivery must replay the first attempt's receipt, not duplicate it"
    );
}

#[test]
fn mpris_completed_replay_at_the_same_terminal_position_the_same_day_is_a_distinct_commit() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let event = MprisMediaEvent {
        player_identity: "VLC".to_owned(),
        track_id: "file:///music/daft_punk/discovery/one_more_time.flac".to_owned(),
        title: "One More Time".to_owned(),
        artist: Some("Daft Punk".to_owned()),
        album: Some("Discovery".to_owned()),
        duration_micros: Some(320000000),
        position_micros: Some(320000000),
        is_completed: true,
    };

    let first_cmd =
        event.to_observation_command(access, sample_observed_at("2026-08-25T10:00:00Z"));
    let second_cmd =
        event.to_observation_command(access, sample_observed_at("2026-08-25T22:00:00Z"));

    assert_ne!(
        first_cmd.operation_id(),
        second_cmd.operation_id(),
        "two same-day completed plays at the same terminal position must not collide"
    );

    let outcome1 = fixture.authorize_and_accept(first_cmd).expect("accepts");
    assert!(matches!(outcome1, AcceptObservationOutcome::Committed(_)));
    let outcome2 = fixture.authorize_and_accept(second_cmd).expect("accepts");
    assert!(
        matches!(outcome2, AcceptObservationOutcome::Committed(_)),
        "the second same-day play must commit as its own observation"
    );
}

#[test]
fn mpris_event_redelivered_with_the_same_observed_at_replays_rather_than_duplicating() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let event = MprisMediaEvent {
        player_identity: "VLC".to_owned(),
        track_id: "file:///music/daft_punk/discovery/one_more_time.flac".to_owned(),
        title: "One More Time".to_owned(),
        artist: Some("Daft Punk".to_owned()),
        album: Some("Discovery".to_owned()),
        duration_micros: Some(320000000),
        position_micros: Some(320000000),
        is_completed: true,
    };

    let first_attempt =
        event.to_observation_command(access, sample_observed_at("2026-08-25T10:00:00Z"));
    let redelivery =
        event.to_observation_command(access, sample_observed_at("2026-08-25T10:00:00Z"));

    assert_eq!(
        first_attempt.operation_id(),
        redelivery.operation_id(),
        "a redelivery with the same observed_at must derive the same operation id"
    );

    let outcome1 = fixture
        .authorize_and_accept(first_attempt)
        .expect("accepts");
    assert!(matches!(outcome1, AcceptObservationOutcome::Committed(_)));
    let outcome2 = fixture.authorize_and_accept(redelivery).expect("replays");
    assert!(
        matches!(outcome2, AcceptObservationOutcome::Replayed(_)),
        "the redelivery must replay the first attempt's receipt, not duplicate it"
    );
}
