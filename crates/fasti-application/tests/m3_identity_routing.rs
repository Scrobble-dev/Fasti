use fasti_application::{plan_purpose_identity_route, ProviderId, PurposeIdentityRouteStatus};
use fasti_domain::{
    AnimeGroupingPreference, ExternalIdentifierClaim, Grain, IdentityRouteKind, RecordId,
    ResolutionIntent,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct GoldenRoute {
    name: String,
    source: String,
    primary_namespace: String,
    primary_value: String,
    imdb_value: String,
    expected_tmdb_namespace: String,
    expected_tmdb_value: String,
}

#[test]
fn pinned_nuvio_anime_routes_use_imdb_for_tmdb_without_rekeying() {
    let fixtures: Vec<GoldenRoute> =
        serde_json::from_str(include_str!("fixtures/nuvio_anime_identity_routes.json"))
            .expect("valid checked-in M3 route fixtures");

    for fixture in fixtures {
        assert!(!fixture.name.is_empty());
        assert!(!fixture.source.is_empty());
        let primary = ExternalIdentifierClaim::try_new(
            fixture.primary_namespace,
            Grain::Release,
            fixture.primary_value,
        )
        .expect("valid primary fixture identifier");
        let imdb =
            ExternalIdentifierClaim::try_new("imdb.title", Grain::Release, fixture.imdb_value)
                .expect("valid IMDb fixture identifier");
        let identifiers = vec![primary.clone(), imdb];

        let plan = plan_purpose_identity_route(
            RecordId::new_v7(),
            ResolutionIntent::MetadataEnrichment,
            ProviderId::try_new("tmdb").expect("TMDB provider"),
            AnimeGroupingPreference::Automatic,
            &identifiers,
        );

        assert_eq!(plan.status(), PurposeIdentityRouteStatus::Selected);
        assert!(plan.known_identifiers().contains(&primary));
        let route = plan.selected_route().expect("selected TMDB alias route");
        assert_eq!(
            route.identifier().namespace(),
            fixture.expected_tmdb_namespace
        );
        assert_eq!(route.identifier().value(), fixture.expected_tmdb_value);
        assert_eq!(route.kind(), IdentityRouteKind::VerifiedAlias);
    }
}
