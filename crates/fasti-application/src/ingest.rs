//! Neutral player webhook and desktop scrobbler ingestion adapters.
//!
//! # Product Boundary
//!
//! **Fasti records. Players play.**
//!
//! Fasti accepts media observations from any player or scrobbling companion
//! (Plex, Jellyfin, Emby, MPRIS Linux/macOS/Windows desktop players).
//! These adapters map vendor-specific payloads into canonical [`AcceptObservationCommand`]
//! instances with deterministically derived operation IDs.

use crate::{
    derive_deterministic_evidence_digest as derive_ingest_evidence_digest,
    derive_deterministic_operation_id as derive_ingest_operation_id, AcceptObservationCommand,
    RequestAccessContext,
};
use fasti_domain::{
    EvidenceId, EvidenceReference, ExternalIdentifierClaim, Grain, ObservedAt, RequestCorrelationId,
};
use serde::{Deserialize, Serialize};

/// Upper bound on GUIDs processed from one Plex webhook payload, so an
/// oversized or hostile payload cannot force unbounded claim-construction work.
const MAX_INGEST_GUIDS: usize = 16;

// ---------------------------------------------------------------------------
// 1. Plex Webhook Adapter
// ---------------------------------------------------------------------------

/// Normalized representation of a Plex Webhook payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlexWebhookPayload {
    pub event: String,
    pub user: bool,
    pub owner: bool,
    #[serde(rename = "Account")]
    pub account: Option<PlexAccount>,
    /// Absent for administrative/library-maintenance events (e.g. database
    /// backup), which are not about a specific media item.
    #[serde(rename = "Metadata")]
    pub metadata: Option<PlexMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlexAccount {
    pub id: u64,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlexMetadata {
    #[serde(rename = "ratingKey")]
    pub rating_key: String,
    #[serde(rename = "type")]
    pub media_type: String,
    pub title: String,
    #[serde(rename = "grandparentTitle")]
    pub grandparent_title: Option<String>,
    #[serde(rename = "parentIndex")]
    pub parent_index: Option<u32>,
    pub index: Option<u32>,
    pub year: Option<u16>,
    pub duration: Option<u64>,
    #[serde(rename = "viewOffset")]
    pub view_offset: Option<u64>,
    pub guid: Option<String>,
    #[serde(rename = "Guid", default)]
    pub guids: Vec<PlexGuidItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlexGuidItem {
    pub id: String,
}

impl PlexWebhookPayload {
    /// Maps a Plex webhook event to an [`AcceptObservationCommand`].
    pub fn to_observation_command(
        &self,
        access: RequestAccessContext,
        observed_at: ObservedAt,
    ) -> Option<AcceptObservationCommand> {
        let is_scrobble_or_play = matches!(
            self.event.as_str(),
            "media.scrobble" | "media.stop" | "media.play"
        );
        if !is_scrobble_or_play {
            return None;
        }

        // Administrative/library-maintenance events omit Metadata entirely;
        // there is no media item to record an observation about.
        let metadata = self.metadata.as_ref()?;

        let grain = match metadata.media_type.as_str() {
            "movie" => Grain::Film,
            "episode" => Grain::Episode,
            "track" => Grain::Track,
            _ => Grain::Custom,
        };

        let mut claims = Vec::new();
        for guid in metadata.guids.iter().take(MAX_INGEST_GUIDS) {
            if let Some((scheme, value)) = guid.id.split_once("://") {
                let ns_str = match scheme {
                    "tmdb" => match grain {
                        Grain::Film => "tmdb.movie",
                        _ => "tmdb.tv",
                    },
                    "imdb" => "imdb.title",
                    "tvdb" => "tvdb.series",
                    // Reject GUID schemes Fasti does not have a mapped
                    // namespace for, rather than minting a domain claim in an
                    // attacker- or vendor-controlled namespace string.
                    _ => continue,
                };
                if let Ok(claim) = ExternalIdentifierClaim::try_new(ns_str, grain, value) {
                    claims.push(claim);
                }
            }
        }

        // Without a real account id, two different users' payloads missing
        // Account would synthesize the same source identity (0) and could
        // collide into one lexeme -- fail closed instead of guessing.
        let account_id = self.account.as_ref()?.id;
        // view_offset distinguishes mid-playback occurrences, but a completed
        // rewatch repeats the same terminal offset as the first watch (both
        // end near the item's full duration). Plex's webhook body carries no
        // other per-occurrence signal, so the caller-supplied observed_at is
        // the only remaining discriminator: it MUST be a stable value
        // (e.g. request receipt time) reused verbatim if this exact webhook
        // delivery is retried, and a fresh value for a genuinely new event,
        // for retry-collapse and rewatch-distinction to both hold.
        let view_offset = metadata.view_offset.unwrap_or(0);
        let occurred_at = observed_at.claim().original();
        let lexeme = format!(
            "plex:account:{account_id}:key:{}:event:{}:offset:{view_offset}:at:{occurred_at}",
            metadata.rating_key, self.event
        );
        let op_id = derive_ingest_operation_id(&lexeme);
        let evidence_digest = derive_ingest_evidence_digest(&lexeme);
        let evidence =
            EvidenceReference::new(EvidenceId::new_v7(), evidence_digest, lexeme.len() as u64);

        let command = AcceptObservationCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            op_id,
            None,
            observed_at,
            evidence,
        )
        .with_identity_clues(claims, Some(grain));

        Some(command)
    }
}

// ---------------------------------------------------------------------------
// 2. Jellyfin & Emby Webhook Adapter
// ---------------------------------------------------------------------------

/// Normalized representation of a Jellyfin/Emby Webhook payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JellyfinWebhookPayload {
    #[serde(rename = "NotificationType")]
    pub notification_type: String,
    #[serde(rename = "ItemType")]
    pub item_type: String,
    #[serde(rename = "ItemId")]
    pub item_id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "SeriesName")]
    pub series_name: Option<String>,
    #[serde(rename = "SeasonNumber")]
    pub season_number: Option<u32>,
    #[serde(rename = "EpisodeNumber")]
    pub episode_number: Option<u32>,
    #[serde(rename = "Year")]
    pub year: Option<u16>,
    #[serde(rename = "Provider_tmdb")]
    pub provider_tmdb: Option<String>,
    #[serde(rename = "Provider_imdb")]
    pub provider_imdb: Option<String>,
    #[serde(rename = "Provider_tvdb")]
    pub provider_tvdb: Option<String>,
    #[serde(rename = "PlayedToCompletion", default)]
    pub played_to_completion: bool,
    #[serde(rename = "UserId")]
    pub user_id: Option<String>,
    #[serde(rename = "PlaybackPositionTicks")]
    pub playback_position_ticks: Option<i64>,
}

impl JellyfinWebhookPayload {
    /// Maps a Jellyfin webhook event to an [`AcceptObservationCommand`].
    pub fn to_observation_command(
        &self,
        access: RequestAccessContext,
        observed_at: ObservedAt,
    ) -> Option<AcceptObservationCommand> {
        let is_relevant = matches!(
            self.notification_type.as_str(),
            "PlaybackStop" | "PlaybackProgress" | "UserDataSaved" | "PlaybackStart"
        );
        if !is_relevant {
            return None;
        }

        let grain = match self.item_type.as_str() {
            "Movie" => Grain::Film,
            "Episode" => Grain::Episode,
            "Audio" | "Track" => Grain::Track,
            _ => Grain::Custom,
        };

        let mut claims = Vec::new();
        if let Some(tmdb_id) = &self.provider_tmdb {
            let ns_key = if grain == Grain::Film {
                "tmdb.movie"
            } else {
                "tmdb.tv"
            };
            if let Ok(claim) = ExternalIdentifierClaim::try_new(ns_key, grain, tmdb_id) {
                claims.push(claim);
            }
        }
        if let Some(imdb_id) = &self.provider_imdb {
            if let Ok(claim) = ExternalIdentifierClaim::try_new("imdb.title", grain, imdb_id) {
                claims.push(claim);
            }
        }
        if let Some(tvdb_id) = &self.provider_tvdb {
            if let Ok(claim) = ExternalIdentifierClaim::try_new("tvdb.series", grain, tvdb_id) {
                claims.push(claim);
            }
        }

        // Without a real user id, two different users' payloads missing
        // UserId would synthesize the same source identity ("default") and
        // could collide into one lexeme -- fail closed instead of guessing.
        let user_str = self.user_id.as_deref()?;
        // playback_position_ticks distinguishes mid-playback occurrences, but a
        // completed rewatch repeats the same terminal tick count as the first
        // watch. Jellyfin's webhook body carries no other per-occurrence
        // signal, so the caller-supplied observed_at is the only remaining
        // discriminator: it MUST be a stable value (e.g. request receipt
        // time) reused verbatim if this exact webhook delivery is retried,
        // and a fresh value for a genuinely new event, for retry-collapse
        // and rewatch-distinction to both hold.
        let position = self.playback_position_ticks.unwrap_or(0);
        let occurred_at = observed_at.claim().original();
        let lexeme = format!(
            "jellyfin:user:{user_str}:item:{}:event:{}:position:{position}:at:{occurred_at}",
            self.item_id, self.notification_type
        );
        let op_id = derive_ingest_operation_id(&lexeme);
        let evidence_digest = derive_ingest_evidence_digest(&lexeme);
        let evidence =
            EvidenceReference::new(EvidenceId::new_v7(), evidence_digest, lexeme.len() as u64);

        let command = AcceptObservationCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            op_id,
            None,
            observed_at,
            evidence,
        )
        .with_identity_clues(claims, Some(grain));

        Some(command)
    }
}

// ---------------------------------------------------------------------------
// 3. Desktop MPRIS & Scrob Companion Adapter
// ---------------------------------------------------------------------------

/// Normalized representation of an MPRIS Linux/Desktop media player event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MprisMediaEvent {
    pub player_identity: String,
    pub track_id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_micros: Option<u64>,
    pub position_micros: Option<u64>,
    pub is_completed: bool,
}

impl MprisMediaEvent {
    /// Maps an MPRIS media event to an [`AcceptObservationCommand`].
    pub fn to_observation_command(
        &self,
        access: RequestAccessContext,
        observed_at: ObservedAt,
    ) -> AcceptObservationCommand {
        // position_micros distinguishes a genuine second play of the same
        // track from the desktop client re-sending the same event on retry,
        // which repeats the position identically. Without it, every
        // intermediate progress tick and every rewatch of a track collapses
        // into the first-ever event for that track_id/completed pair. A
        // completed replay still repeats the same terminal position as the
        // first listen and MPRIS carries no other per-occurrence signal, so
        // the caller-supplied observed_at is the only remaining
        // discriminator: it MUST be a stable value (e.g. request receipt
        // time) reused verbatim if this exact event is retried, and a fresh
        // value for a genuinely new event, for retry-collapse and
        // rewatch-distinction to both hold.
        let position = self.position_micros.unwrap_or(0);
        let occurred_at = observed_at.claim().original();
        let lexeme = format!(
            "mpris:player:{}:track:{}:completed:{}:position:{position}:at:{occurred_at}",
            self.player_identity, self.track_id, self.is_completed
        );
        let op_id = derive_ingest_operation_id(&lexeme);
        let evidence_digest = derive_ingest_evidence_digest(&lexeme);
        let evidence =
            EvidenceReference::new(EvidenceId::new_v7(), evidence_digest, lexeme.len() as u64);

        // track_id is a local player-session identifier (e.g. a DBus object
        // path), not a stable cross-device identity -- record it as
        // provider-scoped evidence only, never as canonical identity.
        let claims =
            ExternalIdentifierClaim::try_new("mpris.trackid", Grain::Track, &self.track_id)
                .map(|claim| vec![claim])
                .unwrap_or_default();

        AcceptObservationCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            op_id,
            None,
            observed_at,
            evidence,
        )
        .with_identity_clues(claims, Some(Grain::Track))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fasti_domain::{
        ClaimedTrust, ClientId, CredentialId, ProfileGrantId, ProfileId, WorkspaceId,
    };

    fn sample_access() -> RequestAccessContext {
        RequestAccessContext::new(
            WorkspaceId::new_v7(),
            ProfileId::new_v7(),
            ClientId::new_v7(),
            CredentialId::new_v7(),
            ProfileGrantId::new_v7(),
            1,
        )
    }

    fn sample_observed_at() -> ObservedAt {
        ObservedAt::parse("2026-08-25T10:00:00Z", ClaimedTrust::DeviceObserved).unwrap()
    }

    #[test]
    fn plex_movie_scrobble_maps_to_canonical_film_command() {
        let payload = PlexWebhookPayload {
            event: "media.scrobble".to_owned(),
            user: true,
            owner: true,
            account: Some(PlexAccount {
                id: 42,
                title: "ryan".to_owned(),
            }),
            metadata: Some(PlexMetadata {
                rating_key: "99182".to_owned(),
                media_type: "movie".to_owned(),
                title: "Princess Mononoke".to_owned(),
                grandparent_title: None,
                parent_index: None,
                index: None,
                year: Some(1997),
                duration: Some(8040000),
                view_offset: Some(8040000),
                guid: Some("plex://movie/5d776824961905001eb9c9e8".to_owned()),
                guids: vec![
                    PlexGuidItem {
                        id: "tmdb://128".to_owned(),
                    },
                    PlexGuidItem {
                        id: "imdb://tt0119698".to_owned(),
                    },
                ],
            }),
        };

        let cmd = payload
            .to_observation_command(sample_access(), sample_observed_at())
            .expect("maps to command");

        assert_eq!(cmd.target_grain(), Some(Grain::Film));
        assert_eq!(cmd.identity_clues().len(), 2);
        assert_eq!(cmd.identity_clues()[0].namespace(), "tmdb.movie");
        assert_eq!(cmd.identity_clues()[0].value(), "128");
        assert_eq!(cmd.identity_clues()[1].namespace(), "imdb.title");
        assert_eq!(cmd.identity_clues()[1].value(), "tt0119698");
    }

    #[test]
    fn jellyfin_episode_webhook_maps_to_canonical_episode_command() {
        let payload = JellyfinWebhookPayload {
            notification_type: "PlaybackStop".to_owned(),
            item_type: "Episode".to_owned(),
            item_id: "jf-item-778".to_owned(),
            name: "Asteroid Blues".to_owned(),
            series_name: Some("Cowboy Bebop".to_owned()),
            season_number: Some(1),
            episode_number: Some(1),
            year: Some(1998),
            provider_tmdb: Some("2490".to_owned()),
            provider_imdb: Some("tt0618966".to_owned()),
            provider_tvdb: Some("76142".to_owned()),
            played_to_completion: true,
            user_id: Some("usr-1".to_owned()),
            playback_position_ticks: Some(14_400_000_000),
        };

        let cmd = payload
            .to_observation_command(sample_access(), sample_observed_at())
            .expect("maps to command");

        assert_eq!(cmd.target_grain(), Some(Grain::Episode));
        assert_eq!(cmd.identity_clues().len(), 3);
        assert_eq!(cmd.identity_clues()[0].namespace(), "tmdb.tv");
        assert_eq!(cmd.identity_clues()[0].value(), "2490");
        assert_eq!(cmd.identity_clues()[1].namespace(), "imdb.title");
        assert_eq!(cmd.identity_clues()[2].namespace(), "tvdb.series");
        assert_eq!(cmd.identity_clues()[2].value(), "76142");
    }

    #[test]
    fn mpris_desktop_track_maps_to_canonical_track_command() {
        let event = MprisMediaEvent {
            player_identity: "Spotify".to_owned(),
            track_id: "spotify:track:3n3Ppam7vgaVa1iaRUc9Lp".to_owned(),
            title: "Mr. Brightside".to_owned(),
            artist: Some("The Killers".to_owned()),
            album: Some("Hot Fuss".to_owned()),
            duration_micros: Some(222000000),
            position_micros: Some(222000000),
            is_completed: true,
        };

        let cmd = event.to_observation_command(sample_access(), sample_observed_at());
        assert_eq!(cmd.target_grain(), Some(Grain::Track));
        assert_eq!(cmd.identity_clues().len(), 1);
        assert_eq!(cmd.identity_clues()[0].namespace(), "mpris.trackid");
        assert_eq!(
            cmd.identity_clues()[0].value(),
            "spotify:track:3n3Ppam7vgaVa1iaRUc9Lp"
        );
    }

    #[test]
    fn mpris_replay_of_the_same_track_produces_the_same_operation_id() {
        let event = MprisMediaEvent {
            player_identity: "Spotify".to_owned(),
            track_id: "spotify:track:3n3Ppam7vgaVa1iaRUc9Lp".to_owned(),
            title: "Mr. Brightside".to_owned(),
            artist: None,
            album: None,
            duration_micros: Some(222_000_000),
            position_micros: Some(222_000_000),
            is_completed: true,
        };
        let cmd_a = event.to_observation_command(sample_access(), sample_observed_at());
        let cmd_b = event.to_observation_command(sample_access(), sample_observed_at());
        assert_eq!(cmd_a.operation_id(), cmd_b.operation_id());
    }

    #[test]
    fn mpris_second_play_of_the_same_track_is_a_distinct_operation() {
        let first_play = MprisMediaEvent {
            player_identity: "Spotify".to_owned(),
            track_id: "spotify:track:3n3Ppam7vgaVa1iaRUc9Lp".to_owned(),
            title: "Mr. Brightside".to_owned(),
            artist: None,
            album: None,
            duration_micros: Some(222_000_000),
            position_micros: Some(222_000_000),
            is_completed: true,
        };
        let second_play = MprisMediaEvent {
            position_micros: Some(1_000_000),
            ..first_play.clone()
        };

        let first_cmd = first_play.to_observation_command(sample_access(), sample_observed_at());
        let second_cmd = second_play.to_observation_command(sample_access(), sample_observed_at());
        assert_ne!(
            first_cmd.operation_id(),
            second_cmd.operation_id(),
            "a rewatch must not be silently deduplicated against the prior play"
        );
    }
}
