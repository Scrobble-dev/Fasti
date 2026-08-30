//! Provider metadata application commands.
//!
//! Provider adapters fetch outside the local transaction, then hand validated
//! claims to this port. The provider coordinate remains evidence attached to a
//! Fasti Record; it never becomes the Record identity.

use crate::{ApplicationResult, RequestAccessContext};
use crate::{ProviderCapabilityState, ProviderId};
use fasti_domain::{
    AnimeGroupingPreference, EnrichmentPolicy, ExternalIdentifierClaim, ExternalIdentifierError,
    FieldClaim, FieldClaimStatus, FieldKey, Grain, IdentityRouteEvidenceKind, IdentityRouteKind,
    MetadataAttribution, MetadataCacheEntry, MetadataCacheReadState, MetadataFieldGroup,
    MetadataLocale, MetadataProjection, MetadataProjectionPolicy, MetadataProviderId,
    MetadataRegion, NamespaceDefinition, NamespaceDefinitionError, NamespaceLicencePosture,
    ProfileId, RatingClaim, RecordId, RequestCorrelationId, ResolutionIntent,
    MAX_EXTERNAL_IDENTIFIER_BYTES, ORIGINAL_TITLE_FIELD_KEY, OVERVIEW_FIELD_KEY, POSTER_FIELD_KEY,
    RELEASE_YEAR_FIELD_KEY, TITLE_FIELD_KEY,
};
use std::{future::Future, pin::Pin};

pub const MAX_PROVIDER_METADATA_FIELDS: usize = 16;
pub const GOOGLE_BOOKS_PROVIDER_ID: &str = "google-books";
pub const TMDB_PROVIDER_ID: &str = "tmdb";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderIdentifierValueKind {
    PositiveDecimal,
    AsciiToken,
}

impl ProviderIdentifierValueKind {
    const fn pattern(self) -> &'static str {
        match self {
            Self::PositiveDecimal => "^[1-9][0-9]*$",
            Self::AsciiToken => "[A-Za-z0-9_-]+",
        }
    }

    fn accepts(self, value: &str) -> bool {
        let trimmed = value.trim();
        value == trimmed
            && !trimmed.is_empty()
            && trimmed.len() <= MAX_EXTERNAL_IDENTIFIER_BYTES
            && match self {
                Self::PositiveDecimal => {
                    let mut bytes = trimmed.bytes();
                    bytes.next().is_some_and(|byte| matches!(byte, b'1'..=b'9'))
                        && bytes.all(|byte| byte.is_ascii_digit())
                }
                Self::AsciiToken => trimmed
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')),
            }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderIdentityMapping {
    provider: &'static str,
    kind: &'static str,
    namespace: &'static str,
    label: &'static str,
    grain: Grain,
    value_kind: ProviderIdentifierValueKind,
}

impl ProviderIdentityMapping {
    pub const fn kind(self) -> &'static str {
        self.kind
    }

    pub const fn namespace(self) -> &'static str {
        self.namespace
    }

    pub const fn grain(self) -> Grain {
        self.grain
    }

    pub fn identifier(
        self,
        value: impl Into<String>,
    ) -> Result<ExternalIdentifierClaim, ExternalIdentifierError> {
        let value = value.into();
        if !self.accepts_value(&value) {
            return Err(ExternalIdentifierError::InvalidValue);
        }
        ExternalIdentifierClaim::try_new(self.namespace, self.grain, value)
    }

    pub fn accepts_value(self, value: &str) -> bool {
        self.value_kind.accepts(value)
    }

    pub fn namespace_definition(self) -> Result<NamespaceDefinition, NamespaceDefinitionError> {
        NamespaceDefinition::try_new(
            self.namespace,
            self.label,
            [self.grain],
            self.value_kind.pattern(),
            "identity",
            NamespaceLicencePosture::IdentifiersOnly,
        )
    }
}

const PROVIDER_IDENTITY_MAPPINGS: &[ProviderIdentityMapping] = &[
    ProviderIdentityMapping {
        provider: GOOGLE_BOOKS_PROVIDER_ID,
        kind: "book",
        namespace: "googlebooks.volume",
        label: "Google Books Volume",
        grain: Grain::Edition,
        value_kind: ProviderIdentifierValueKind::AsciiToken,
    },
    ProviderIdentityMapping {
        provider: TMDB_PROVIDER_ID,
        kind: "movie",
        namespace: "tmdb.movie",
        label: "TMDB Movie",
        grain: Grain::Film,
        value_kind: ProviderIdentifierValueKind::PositiveDecimal,
    },
    ProviderIdentityMapping {
        provider: TMDB_PROVIDER_ID,
        kind: "show",
        namespace: "tmdb.tv",
        label: "TMDB TV",
        grain: Grain::Series,
        value_kind: ProviderIdentifierValueKind::PositiveDecimal,
    },
];

pub fn provider_identity_mapping(provider: &str, kind: &str) -> Option<ProviderIdentityMapping> {
    PROVIDER_IDENTITY_MAPPINGS
        .iter()
        .copied()
        .find(|mapping| mapping.provider == provider && mapping.kind == kind)
}

pub fn provider_identity_mapping_for_grain(
    provider: &str,
    grain: Grain,
) -> Option<ProviderIdentityMapping> {
    PROVIDER_IDENTITY_MAPPINGS
        .iter()
        .copied()
        .find(|mapping| mapping.provider == provider && mapping.grain == grain)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurposeIdentityRoute {
    identifier: ExternalIdentifierClaim,
    kind: IdentityRouteKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityRouteEvidence {
    identifier: ExternalIdentifierClaim,
    kind: IdentityRouteEvidenceKind,
}

impl IdentityRouteEvidence {
    pub const fn new(identifier: ExternalIdentifierClaim, kind: IdentityRouteEvidenceKind) -> Self {
        Self { identifier, kind }
    }

    pub const fn direct(identifier: ExternalIdentifierClaim) -> Self {
        Self::new(identifier, IdentityRouteEvidenceKind::Direct)
    }

    pub const fn identifier(&self) -> &ExternalIdentifierClaim {
        &self.identifier
    }

    pub const fn kind(&self) -> IdentityRouteEvidenceKind {
        self.kind
    }
}

impl PurposeIdentityRoute {
    pub const fn identifier(&self) -> &ExternalIdentifierClaim {
        &self.identifier
    }

    pub const fn kind(&self) -> IdentityRouteKind {
        self.kind
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurposeIdentityRouteStatus {
    Selected,
    Missing,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurposeIdentityRoutePlan {
    intent: ResolutionIntent,
    target_provider: ProviderId,
    status: PurposeIdentityRouteStatus,
    known_identifiers: Vec<ExternalIdentifierClaim>,
    candidate_routes: Vec<PurposeIdentityRoute>,
    selected_route: Option<PurposeIdentityRoute>,
}

impl PurposeIdentityRoutePlan {
    pub const fn intent(&self) -> ResolutionIntent {
        self.intent
    }

    pub const fn target_provider(&self) -> &ProviderId {
        &self.target_provider
    }

    pub const fn status(&self) -> PurposeIdentityRouteStatus {
        self.status
    }

    pub fn known_identifiers(&self) -> &[ExternalIdentifierClaim] {
        &self.known_identifiers
    }

    pub fn candidate_routes(&self) -> &[PurposeIdentityRoute] {
        &self.candidate_routes
    }

    pub const fn selected_route(&self) -> Option<&PurposeIdentityRoute> {
        self.selected_route.as_ref()
    }

    pub fn nuvio_content_id(&self) -> Option<String> {
        if self.intent != ResolutionIntent::NuvioExport {
            return None;
        }
        let identifier = self.selected_route()?.identifier();
        let prefix = match identifier.namespace() {
            "imdb.title" => return Some(identifier.value().to_owned()),
            "tmdb.movie" | "tmdb.tv" => "tmdb",
            "tvdb.movie" | "tvdb.series" => "tvdb",
            "mal.anime" => "mal",
            "anidb.anime" => "anidb",
            "anilist.anime" => "anilist",
            "kitsu.anime" => "kitsu",
            "simkl.anime" => "simkl",
            _ => return None,
        };
        Some(format!("{prefix}:{}", identifier.value()))
    }
}

fn nuvio_standard_route_priority(namespace: &str, grain: Grain) -> Option<u8> {
    match (namespace, grain) {
        ("imdb.title", Grain::Film | Grain::Series | Grain::Release) => Some(0),
        ("tmdb.movie", Grain::Film | Grain::Release)
        | ("tmdb.tv", Grain::Series | Grain::Release) => Some(1),
        ("tvdb.movie", Grain::Film | Grain::Release)
        | ("tvdb.series", Grain::Series | Grain::Release) => Some(2),
        ("mal.anime", Grain::Release) => Some(3),
        ("anidb.anime", Grain::Release) => Some(4),
        ("anilist.anime", Grain::Release) => Some(5),
        ("kitsu.anime", Grain::Release) => Some(6),
        ("simkl.anime", Grain::Release) => Some(7),
        _ => None,
    }
}

fn route_priority(
    intent: ResolutionIntent,
    target_provider: &str,
    anime_preference: AnimeGroupingPreference,
    evidence: &IdentityRouteEvidence,
) -> Option<(u8, IdentityRouteKind)> {
    let identifier = evidence.identifier();
    let namespace = identifier.namespace();
    let grain = identifier.grain();
    match (intent, target_provider, namespace, grain, evidence.kind()) {
        (
            ResolutionIntent::MetadataLookup
            | ResolutionIntent::MetadataEnrichment
            | ResolutionIntent::DisplayProjection,
            "tmdb",
            "tmdb.movie",
            Grain::Film,
            IdentityRouteEvidenceKind::Direct,
        )
        | (
            ResolutionIntent::MetadataLookup
            | ResolutionIntent::MetadataEnrichment
            | ResolutionIntent::DisplayProjection,
            "tmdb",
            "tmdb.tv",
            Grain::Series,
            IdentityRouteEvidenceKind::Direct,
        ) => Some((0, IdentityRouteKind::ProviderNative)),
        (
            ResolutionIntent::MetadataLookup
            | ResolutionIntent::MetadataEnrichment
            | ResolutionIntent::DisplayProjection,
            "tmdb",
            "imdb.title",
            Grain::Film | Grain::Series | Grain::Release,
            IdentityRouteEvidenceKind::Direct,
        ) => Some((1, IdentityRouteKind::VerifiedAlias)),
        (
            ResolutionIntent::MetadataLookup
            | ResolutionIntent::MetadataEnrichment
            | ResolutionIntent::DisplayProjection,
            "tmdb",
            "tvdb.movie" | "tvdb.series" | "wikidata",
            Grain::Film | Grain::Series | Grain::Release,
            IdentityRouteEvidenceKind::Direct,
        ) => Some((2, IdentityRouteKind::VerifiedAlias)),
        (
            ResolutionIntent::MetadataLookup
            | ResolutionIntent::MetadataEnrichment
            | ResolutionIntent::DisplayProjection,
            "tmdb",
            "tmdb.movie" | "tmdb.tv",
            Grain::Film | Grain::Series,
            IdentityRouteEvidenceKind::AcceptedCrosswalk,
        ) => Some((3, IdentityRouteKind::AcceptedCrosswalk)),
        (
            ResolutionIntent::MetadataLookup
            | ResolutionIntent::MetadataEnrichment
            | ResolutionIntent::DisplayProjection,
            "google-books",
            "googlebooks.volume",
            Grain::Edition,
            IdentityRouteEvidenceKind::Direct,
        )
        | (
            ResolutionIntent::TrackerRead | ResolutionIntent::TrackerWrite,
            "mal",
            "mal.anime",
            Grain::Release,
            IdentityRouteEvidenceKind::Direct,
        )
        | (
            ResolutionIntent::TrackerRead | ResolutionIntent::TrackerWrite,
            "kitsu",
            "kitsu.anime",
            Grain::Release,
            IdentityRouteEvidenceKind::Direct,
        ) => Some((0, IdentityRouteKind::ProviderNative)),
        (ResolutionIntent::NuvioExport, "nuvio", _, _, IdentityRouteEvidenceKind::Direct) => {
            let release = |expected| namespace == expected && grain == Grain::Release;
            let priority = match anime_preference {
                AnimeGroupingPreference::GroupByTvWork | AnimeGroupingPreference::Automatic => {
                    nuvio_standard_route_priority(namespace, grain)?
                }
                AnimeGroupingPreference::KeepMalReleasesSeparate => {
                    if release("mal.anime") {
                        0
                    } else if release("kitsu.anime") {
                        1
                    } else if release("anidb.anime") {
                        2
                    } else {
                        nuvio_standard_route_priority(namespace, grain)? + 3
                    }
                }
                AnimeGroupingPreference::KeepKitsuReleasesSeparate => {
                    if release("kitsu.anime") {
                        0
                    } else if release("mal.anime") {
                        1
                    } else if release("anidb.anime") {
                        2
                    } else {
                        nuvio_standard_route_priority(namespace, grain)? + 3
                    }
                }
            };
            Some((priority, IdentityRouteKind::ProviderNative))
        }
        _ => None,
    }
}

/// Plan one provider route without changing a Record or Chronicle history.
///
/// Unsupported identifiers remain visible in `known_identifiers`. Multiple
/// identifiers at the best accepted priority fail closed as ambiguous.
pub fn plan_purpose_identity_route(
    intent: ResolutionIntent,
    target_provider: ProviderId,
    anime_preference: AnimeGroupingPreference,
    identifiers: &[ExternalIdentifierClaim],
) -> PurposeIdentityRoutePlan {
    let evidence = identifiers
        .iter()
        .cloned()
        .map(IdentityRouteEvidence::direct)
        .collect::<Vec<_>>();
    plan_purpose_identity_route_with_evidence(intent, target_provider, anime_preference, &evidence)
}

pub fn plan_purpose_identity_route_with_evidence(
    intent: ResolutionIntent,
    target_provider: ProviderId,
    anime_preference: AnimeGroupingPreference,
    evidence: &[IdentityRouteEvidence],
) -> PurposeIdentityRoutePlan {
    let mut known_identifiers = evidence
        .iter()
        .map(|item| item.identifier().clone())
        .collect::<Vec<_>>();
    known_identifiers.sort_by(|left, right| {
        (left.namespace(), left.grain(), left.value()).cmp(&(
            right.namespace(),
            right.grain(),
            right.value(),
        ))
    });
    known_identifiers.dedup();

    let mut candidates = evidence
        .iter()
        .filter_map(|evidence| {
            route_priority(intent, target_provider.as_str(), anime_preference, evidence).map(
                |(priority, kind)| {
                    (
                        priority,
                        PurposeIdentityRoute {
                            identifier: evidence.identifier().clone(),
                            kind,
                        },
                    )
                },
            )
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|(left_priority, left), (right_priority, right)| {
        (
            left_priority,
            left.identifier.namespace(),
            left.identifier.grain(),
            left.identifier.value(),
        )
            .cmp(&(
                right_priority,
                right.identifier.namespace(),
                right.identifier.grain(),
                right.identifier.value(),
            ))
    });
    candidates.dedup();

    let best_priority = candidates.first().map(|(priority, _)| *priority);
    let best = candidates
        .iter()
        .filter(|(priority, _)| Some(*priority) == best_priority)
        .map(|(_, route)| route)
        .collect::<Vec<_>>();
    let (status, selected_route) = match best.as_slice() {
        [] => (PurposeIdentityRouteStatus::Missing, None),
        [selected] => (
            PurposeIdentityRouteStatus::Selected,
            Some((*selected).clone()),
        ),
        _ => (PurposeIdentityRouteStatus::Ambiguous, None),
    };

    PurposeIdentityRoutePlan {
        intent,
        target_provider,
        status,
        known_identifiers,
        candidate_routes: candidates.into_iter().map(|(_, route)| route).collect(),
        selected_route,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimeGroupingRecordPreview {
    record_id: RecordId,
    previous_status: PurposeIdentityRouteStatus,
    proposed_status: PurposeIdentityRouteStatus,
    previous_route: Option<PurposeIdentityRoute>,
    proposed_route: Option<PurposeIdentityRoute>,
    route_changed: bool,
    possible_season_regrouping: bool,
}

impl AnimeGroupingRecordPreview {
    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn previous_status(&self) -> PurposeIdentityRouteStatus {
        self.previous_status
    }

    pub const fn proposed_status(&self) -> PurposeIdentityRouteStatus {
        self.proposed_status
    }

    pub const fn previous_route(&self) -> Option<&PurposeIdentityRoute> {
        self.previous_route.as_ref()
    }

    pub const fn proposed_route(&self) -> Option<&PurposeIdentityRoute> {
        self.proposed_route.as_ref()
    }

    pub const fn route_changed(&self) -> bool {
        self.route_changed
    }

    pub const fn unresolved(&self) -> bool {
        !matches!(self.proposed_status, PurposeIdentityRouteStatus::Selected)
    }

    pub const fn possible_season_regrouping(&self) -> bool {
        self.possible_season_regrouping
    }
}

const fn groups_by_tv_work(preference: AnimeGroupingPreference) -> bool {
    matches!(
        preference,
        AnimeGroupingPreference::GroupByTvWork | AnimeGroupingPreference::Automatic
    )
}

/// Compare one Record's outward Nuvio route without mutating identity or history.
pub fn preview_anime_grouping_change_for_record(
    record_id: RecordId,
    previous_preference: AnimeGroupingPreference,
    proposed_preference: AnimeGroupingPreference,
    identifiers: &[ExternalIdentifierClaim],
) -> AnimeGroupingRecordPreview {
    let nuvio = ProviderId::try_new("nuvio").expect("the fixed Nuvio provider ID is valid");
    let previous = plan_purpose_identity_route(
        ResolutionIntent::NuvioExport,
        nuvio.clone(),
        previous_preference,
        identifiers,
    );
    let proposed = plan_purpose_identity_route(
        ResolutionIntent::NuvioExport,
        nuvio,
        proposed_preference,
        identifiers,
    );
    let route_changed =
        previous.status != proposed.status || previous.selected_route != proposed.selected_route;

    AnimeGroupingRecordPreview {
        record_id,
        previous_status: previous.status,
        proposed_status: proposed.status,
        previous_route: previous.selected_route,
        proposed_route: proposed.selected_route,
        route_changed,
        possible_season_regrouping: route_changed
            && groups_by_tv_work(previous_preference) != groups_by_tv_work(proposed_preference),
    }
}

pub fn metadata_field_group(field_key: &FieldKey) -> Option<MetadataFieldGroup> {
    match field_key.as_str() {
        TITLE_FIELD_KEY | ORIGINAL_TITLE_FIELD_KEY => Some(MetadataFieldGroup::BasicInfo),
        OVERVIEW_FIELD_KEY => Some(MetadataFieldGroup::Details),
        POSTER_FIELD_KEY => Some(MetadataFieldGroup::Artwork),
        RELEASE_YEAR_FIELD_KEY => Some(MetadataFieldGroup::ReleaseDates),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderMetadataField {
    field_key: FieldKey,
    claim: FieldClaim,
}

impl ProviderMetadataField {
    pub const fn new(field_key: FieldKey, claim: FieldClaim) -> Self {
        Self { field_key, claim }
    }

    pub const fn field_key(&self) -> &FieldKey {
        &self.field_key
    }

    pub const fn claim(&self) -> &FieldClaim {
        &self.claim
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateProviderRecordCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    grain: Grain,
    identifier: ExternalIdentifierClaim,
    fields: Vec<ProviderMetadataField>,
}

impl CreateProviderRecordCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        grain: Grain,
        identifier: ExternalIdentifierClaim,
        fields: Vec<ProviderMetadataField>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            grain,
            identifier,
            fields,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn grain(&self) -> Grain {
        self.grain
    }

    pub const fn identifier(&self) -> &ExternalIdentifierClaim {
        &self.identifier
    }

    pub fn fields(&self) -> &[ProviderMetadataField] {
        &self.fields
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplyProviderMetadataCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    record_id: RecordId,
    identifier: ExternalIdentifierClaim,
    fields: Vec<ProviderMetadataField>,
}

impl ApplyProviderMetadataCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        record_id: RecordId,
        identifier: ExternalIdentifierClaim,
        fields: Vec<ProviderMetadataField>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            record_id,
            identifier,
            fields,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn identifier(&self) -> &ExternalIdentifierClaim {
        &self.identifier
    }

    pub fn fields(&self) -> &[ProviderMetadataField] {
        &self.fields
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateProviderRecordOutcome {
    record_id: RecordId,
    grain: Grain,
}

impl CreateProviderRecordOutcome {
    pub const fn new(record_id: RecordId, grain: Grain) -> Self {
        Self { record_id, grain }
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn grain(&self) -> Grain {
        self.grain
    }
}

pub trait ProviderMetadataPort: Send + Sync {
    fn create_provider_record(
        &self,
        command: CreateProviderRecordCommand,
    ) -> ApplicationResult<CreateProviderRecordOutcome>;

    fn apply_provider_metadata(
        &self,
        command: ApplyProviderMetadataCommand,
    ) -> ApplicationResult<()>;
}

/// Maximum number of independently governed metadata groups one refresh may
/// request. The domain currently defines fewer groups, but this explicit wire
/// bound prevents an adapter from accepting an unbounded allocation if that
/// list grows.
pub const MAX_METADATA_REFRESH_FIELD_GROUPS: usize = 32;

/// Whether a refresh may use an exact fresh cache partition or must revalidate
/// it with the provider. Neither mode permits a stale partition to masquerade
/// as a successful provider refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataRefreshMode {
    PreferCache,
    Revalidate,
}

/// Authenticated request to refresh immutable provider claims for one Record.
///
/// Provider source identifiers, routes, credential-reference versions,
/// settings fingerprints, and cache partition digests are deliberately absent:
/// the service derives them from durable identity and provider configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshMetadataClaimsCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    operation_id: fasti_domain::OperationId,
    record_id: RecordId,
    provider_id: MetadataProviderId,
    field_groups: Vec<MetadataFieldGroup>,
    locale: Option<MetadataLocale>,
    region: Option<MetadataRegion>,
    mode: MetadataRefreshMode,
}

impl RefreshMetadataClaimsCommand {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        operation_id: fasti_domain::OperationId,
        record_id: RecordId,
        provider_id: MetadataProviderId,
        mut field_groups: Vec<MetadataFieldGroup>,
        locale: Option<MetadataLocale>,
        region: Option<MetadataRegion>,
        mode: MetadataRefreshMode,
    ) -> Self {
        field_groups.sort_unstable();
        field_groups.dedup();
        Self {
            correlation_id,
            access,
            operation_id,
            record_id,
            provider_id,
            field_groups,
            locale,
            region,
            mode,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn operation_id(&self) -> fasti_domain::OperationId {
        self.operation_id
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn provider_id(&self) -> &MetadataProviderId {
        &self.provider_id
    }

    pub fn field_groups(&self) -> &[MetadataFieldGroup] {
        &self.field_groups
    }

    pub const fn locale(&self) -> Option<&MetadataLocale> {
        self.locale.as_ref()
    }

    pub const fn region(&self) -> Option<&MetadataRegion> {
        self.region.as_ref()
    }

    pub const fn mode(&self) -> MetadataRefreshMode {
        self.mode
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldClaimView {
    claim: FieldClaim,
    status: FieldClaimStatus,
}

impl FieldClaimView {
    pub const fn new(claim: FieldClaim, status: FieldClaimStatus) -> Self {
        Self { claim, status }
    }

    pub const fn claim(&self) -> &FieldClaim {
        &self.claim
    }

    pub const fn status(&self) -> FieldClaimStatus {
        self.status
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RatingClaimView {
    claim: RatingClaim,
    status: FieldClaimStatus,
}

impl RatingClaimView {
    pub const fn new(claim: RatingClaim, status: FieldClaimStatus) -> Self {
        Self { claim, status }
    }

    pub const fn claim(&self) -> &RatingClaim {
        &self.claim
    }

    pub const fn status(&self) -> FieldClaimStatus {
        self.status
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataCacheReadView {
    entry: MetadataCacheEntry,
    state: MetadataCacheReadState,
}

impl MetadataCacheReadView {
    pub const fn new(entry: MetadataCacheEntry, state: MetadataCacheReadState) -> Self {
        Self { entry, state }
    }

    pub const fn entry(&self) -> &MetadataCacheEntry {
        &self.entry
    }

    pub const fn state(&self) -> MetadataCacheReadState {
        self.state
    }
}

/// Complete safe result of one refresh. Failed refreshes return a problem and
/// must leave prior valid claims available through projection reads.
#[derive(Debug, Clone, PartialEq)]
pub struct RefreshMetadataClaimsOutcome {
    field_claims: Vec<FieldClaimView>,
    rating_claims: Vec<RatingClaimView>,
    projections: Vec<MetadataProjection>,
    cache_entries: Vec<MetadataCacheReadView>,
    attributions: Vec<MetadataAttribution>,
}

impl RefreshMetadataClaimsOutcome {
    pub fn new(
        field_claims: Vec<FieldClaimView>,
        rating_claims: Vec<RatingClaimView>,
        projections: Vec<MetadataProjection>,
        cache_entries: Vec<MetadataCacheReadView>,
        attributions: Vec<MetadataAttribution>,
    ) -> Self {
        Self {
            field_claims,
            rating_claims,
            projections,
            cache_entries,
            attributions,
        }
    }

    pub fn field_claims(&self) -> &[FieldClaimView] {
        &self.field_claims
    }

    pub fn rating_claims(&self) -> &[RatingClaimView] {
        &self.rating_claims
    }

    pub fn projections(&self) -> &[MetadataProjection] {
        &self.projections
    }

    pub fn cache_entries(&self) -> &[MetadataCacheReadView] {
        &self.cache_entries
    }

    pub fn attributions(&self) -> &[MetadataAttribution] {
        &self.attributions
    }
}

pub type MetadataRefreshFuture<'a> =
    Pin<Box<dyn Future<Output = ApplicationResult<RefreshMetadataClaimsOutcome>> + Send + 'a>>;

/// Provider orchestration boundary. Implementations must re-authorize the
/// presented access context immediately before committing immutable claims and
/// cache references. Provider I/O occurs outside that local transaction.
pub trait MetadataClaimRefreshService: Send + Sync {
    fn authorize_and_refresh(
        &self,
        command: RefreshMetadataClaimsCommand,
    ) -> MetadataRefreshFuture<'_>;
}

/// Authorized, immutable inputs needed for provider I/O.
///
/// The store resolves the exact provider identifier from the requested Record;
/// callers must not discover it by listing or guessing across a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedMetadataRefresh {
    record_id: RecordId,
    grain: Grain,
    identifier: ExternalIdentifierClaim,
    field_groups: Vec<MetadataFieldGroup>,
    settings_fingerprint: fasti_domain::Sha256Digest,
}

impl PreparedMetadataRefresh {
    pub fn new(
        record_id: RecordId,
        grain: Grain,
        identifier: ExternalIdentifierClaim,
        field_groups: Vec<MetadataFieldGroup>,
        settings_fingerprint: fasti_domain::Sha256Digest,
    ) -> Self {
        Self {
            record_id,
            grain,
            identifier,
            field_groups,
            settings_fingerprint,
        }
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn grain(&self) -> Grain {
        self.grain
    }

    pub const fn identifier(&self) -> &ExternalIdentifierClaim {
        &self.identifier
    }

    pub fn field_groups(&self) -> &[MetadataFieldGroup] {
        &self.field_groups
    }

    pub const fn settings_fingerprint(&self) -> &fasti_domain::Sha256Digest {
        &self.settings_fingerprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareMetadataRefreshCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    record_id: RecordId,
    provider_id: MetadataProviderId,
    field_groups: Vec<MetadataFieldGroup>,
}

impl PrepareMetadataRefreshCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        record_id: RecordId,
        provider_id: MetadataProviderId,
        field_groups: Vec<MetadataFieldGroup>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            record_id,
            provider_id,
            field_groups,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn provider_id(&self) -> &MetadataProviderId {
        &self.provider_id
    }

    pub fn field_groups(&self) -> &[MetadataFieldGroup] {
        &self.field_groups
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommitMetadataRefreshCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    operation_id: fasti_domain::OperationId,
    semantic_digest: fasti_domain::Sha256Digest,
    prepared: PreparedMetadataRefresh,
    provider_id: MetadataProviderId,
    expected_provider_state: ProviderCapabilityState,
    fields: Vec<ProviderMetadataField>,
    ratings: Vec<RatingClaim>,
    cache_entries: Vec<MetadataCacheEntry>,
    attribution: MetadataAttribution,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadCachedMetadataRefreshCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    prepared: PreparedMetadataRefresh,
    cache_keys: Vec<fasti_domain::MetadataCacheKey>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadMetadataRefreshReceiptCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    operation_id: fasti_domain::OperationId,
    semantic_digest: fasti_domain::Sha256Digest,
    record_id: RecordId,
    provider_id: MetadataProviderId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommitMetadataRefreshReceiptCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    operation_id: fasti_domain::OperationId,
    semantic_digest: fasti_domain::Sha256Digest,
    record_id: RecordId,
    provider_id: MetadataProviderId,
    outcome: RefreshMetadataClaimsOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkMetadataRefreshUnavailableCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    prepared: PreparedMetadataRefresh,
    provider_id: MetadataProviderId,
}

impl MarkMetadataRefreshUnavailableCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        prepared: PreparedMetadataRefresh,
        provider_id: MetadataProviderId,
    ) -> Self {
        Self {
            correlation_id,
            access,
            prepared,
            provider_id,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
    pub const fn prepared(&self) -> &PreparedMetadataRefresh {
        &self.prepared
    }
    pub const fn provider_id(&self) -> &MetadataProviderId {
        &self.provider_id
    }
}

impl ReadCachedMetadataRefreshCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        prepared: PreparedMetadataRefresh,
        cache_keys: Vec<fasti_domain::MetadataCacheKey>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            prepared,
            cache_keys,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
    pub const fn prepared(&self) -> &PreparedMetadataRefresh {
        &self.prepared
    }
    pub fn cache_keys(&self) -> &[fasti_domain::MetadataCacheKey] {
        &self.cache_keys
    }
}

impl ReadMetadataRefreshReceiptCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        operation_id: fasti_domain::OperationId,
        semantic_digest: fasti_domain::Sha256Digest,
        record_id: RecordId,
        provider_id: MetadataProviderId,
    ) -> Self {
        Self {
            correlation_id,
            access,
            operation_id,
            semantic_digest,
            record_id,
            provider_id,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
    pub const fn operation_id(&self) -> fasti_domain::OperationId {
        self.operation_id
    }
    pub const fn semantic_digest(&self) -> &fasti_domain::Sha256Digest {
        &self.semantic_digest
    }
    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }
    pub const fn provider_id(&self) -> &MetadataProviderId {
        &self.provider_id
    }
}

impl CommitMetadataRefreshReceiptCommand {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        operation_id: fasti_domain::OperationId,
        semantic_digest: fasti_domain::Sha256Digest,
        record_id: RecordId,
        provider_id: MetadataProviderId,
        outcome: RefreshMetadataClaimsOutcome,
    ) -> Self {
        Self {
            correlation_id,
            access,
            operation_id,
            semantic_digest,
            record_id,
            provider_id,
            outcome,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
    pub const fn operation_id(&self) -> fasti_domain::OperationId {
        self.operation_id
    }
    pub const fn semantic_digest(&self) -> &fasti_domain::Sha256Digest {
        &self.semantic_digest
    }
    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }
    pub const fn provider_id(&self) -> &MetadataProviderId {
        &self.provider_id
    }
    pub const fn outcome(&self) -> &RefreshMetadataClaimsOutcome {
        &self.outcome
    }
}

impl CommitMetadataRefreshCommand {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        operation_id: fasti_domain::OperationId,
        semantic_digest: fasti_domain::Sha256Digest,
        prepared: PreparedMetadataRefresh,
        provider_id: MetadataProviderId,
        expected_provider_state: ProviderCapabilityState,
        fields: Vec<ProviderMetadataField>,
        ratings: Vec<RatingClaim>,
        cache_entries: Vec<MetadataCacheEntry>,
        attribution: MetadataAttribution,
    ) -> Self {
        Self {
            correlation_id,
            access,
            operation_id,
            semantic_digest,
            prepared,
            provider_id,
            expected_provider_state,
            fields,
            ratings,
            cache_entries,
            attribution,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
    pub const fn operation_id(&self) -> fasti_domain::OperationId {
        self.operation_id
    }
    pub const fn semantic_digest(&self) -> &fasti_domain::Sha256Digest {
        &self.semantic_digest
    }
    pub const fn prepared(&self) -> &PreparedMetadataRefresh {
        &self.prepared
    }
    pub const fn provider_id(&self) -> &MetadataProviderId {
        &self.provider_id
    }
    pub const fn expected_provider_state(&self) -> &ProviderCapabilityState {
        &self.expected_provider_state
    }
    pub fn fields(&self) -> &[ProviderMetadataField] {
        &self.fields
    }
    pub fn ratings(&self) -> &[RatingClaim] {
        &self.ratings
    }
    pub fn cache_entries(&self) -> &[MetadataCacheEntry] {
        &self.cache_entries
    }
    pub const fn attribution(&self) -> &MetadataAttribution {
        &self.attribution
    }
}

/// Transactional store half of metadata refresh orchestration.
///
/// Preparation and commit both re-authorize. Commit must verify that the
/// Record, provider identifier, and settings fingerprint still equal the
/// prepared values before writing all claims, cache references, projections,
/// and attribution in one transaction.
pub trait MetadataRefreshPersistencePort: Send + Sync {
    fn authorize_and_prepare_refresh(
        &self,
        command: PrepareMetadataRefreshCommand,
    ) -> ApplicationResult<PreparedMetadataRefresh>;

    /// Return a result only when every exact requested cache partition is
    /// fresh. Missing, stale, invalidated, or mismatched partitions return
    /// `Ok(None)` without mutation.
    fn authorize_and_read_cached_refresh(
        &self,
        command: ReadCachedMetadataRefreshCommand,
    ) -> ApplicationResult<Option<RefreshMetadataClaimsOutcome>>;

    fn authorize_and_read_refresh_receipt(
        &self,
        command: ReadMetadataRefreshReceiptCommand,
    ) -> ApplicationResult<Option<RefreshMetadataClaimsOutcome>>;

    fn authorize_and_commit_refresh_receipt(
        &self,
        command: CommitMetadataRefreshReceiptCommand,
    ) -> ApplicationResult<RefreshMetadataClaimsOutcome>;

    fn authorize_and_mark_refresh_unavailable(
        &self,
        command: MarkMetadataRefreshUnavailableCommand,
    ) -> ApplicationResult<()>;

    fn authorize_and_commit_refresh(
        &self,
        command: CommitMetadataRefreshCommand,
    ) -> ApplicationResult<RefreshMetadataClaimsOutcome>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadMetadataProjectionQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    record_id: RecordId,
    offline: bool,
}

impl ReadMetadataProjectionQuery {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        record_id: RecordId,
        offline: bool,
    ) -> Self {
        Self {
            correlation_id,
            access,
            record_id,
            offline,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn offline(&self) -> bool {
        self.offline
    }
}

/// One authenticated profile's selected metadata for one Record. Raw provider
/// bodies and credentials never cross this boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct MetadataProjectionView {
    profile_id: ProfileId,
    record_id: RecordId,
    enrichment_policy: EnrichmentPolicy,
    fields: Vec<MetadataProjection>,
    ratings: Vec<RatingClaimView>,
    cache_entries: Vec<MetadataCacheReadView>,
    attributions: Vec<MetadataAttribution>,
}

impl MetadataProjectionView {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        profile_id: ProfileId,
        record_id: RecordId,
        enrichment_policy: EnrichmentPolicy,
        fields: Vec<MetadataProjection>,
        ratings: Vec<RatingClaimView>,
        cache_entries: Vec<MetadataCacheReadView>,
        attributions: Vec<MetadataAttribution>,
    ) -> Self {
        Self {
            profile_id,
            record_id,
            enrichment_policy,
            fields,
            ratings,
            cache_entries,
            attributions,
        }
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn enrichment_policy(&self) -> &EnrichmentPolicy {
        &self.enrichment_policy
    }

    pub fn fields(&self) -> &[MetadataProjection] {
        &self.fields
    }

    pub fn ratings(&self) -> &[RatingClaimView] {
        &self.ratings
    }

    pub fn cache_entries(&self) -> &[MetadataCacheReadView] {
        &self.cache_entries
    }

    pub fn attributions(&self) -> &[MetadataAttribution] {
        &self.attributions
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataOverrideMutation {
    Set {
        record_id: RecordId,
        field_key: FieldKey,
        value: String,
    },
    Clear {
        record_id: RecordId,
        field_key: FieldKey,
    },
}

impl MetadataOverrideMutation {
    pub const fn record_id(&self) -> RecordId {
        match self {
            Self::Set { record_id, .. } | Self::Clear { record_id, .. } => *record_id,
        }
    }

    pub const fn field_key(&self) -> &FieldKey {
        match self {
            Self::Set { field_key, .. } | Self::Clear { field_key, .. } => field_key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigureMetadataProjectionCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    projection_policy: MetadataProjectionPolicy,
    region: Option<MetadataRegion>,
    enabled_field_groups: Vec<MetadataFieldGroup>,
    override_mutations: Vec<MetadataOverrideMutation>,
}

impl ConfigureMetadataProjectionCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        projection_policy: MetadataProjectionPolicy,
        region: Option<MetadataRegion>,
        mut enabled_field_groups: Vec<MetadataFieldGroup>,
        override_mutations: Vec<MetadataOverrideMutation>,
    ) -> Self {
        enabled_field_groups.sort_unstable();
        enabled_field_groups.dedup();
        Self {
            correlation_id,
            access,
            projection_policy,
            region,
            enabled_field_groups,
            override_mutations,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn projection_policy(&self) -> &MetadataProjectionPolicy {
        &self.projection_policy
    }

    pub const fn region(&self) -> Option<&MetadataRegion> {
        self.region.as_ref()
    }

    pub fn enabled_field_groups(&self) -> &[MetadataFieldGroup] {
        &self.enabled_field_groups
    }

    pub fn override_mutations(&self) -> &[MetadataOverrideMutation] {
        &self.override_mutations
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigureMetadataProjectionOutcome {
    enrichment_policy: EnrichmentPolicy,
    invalidated_cache_entries: u32,
}

impl ConfigureMetadataProjectionOutcome {
    pub const fn new(enrichment_policy: EnrichmentPolicy, invalidated_cache_entries: u32) -> Self {
        Self {
            enrichment_policy,
            invalidated_cache_entries,
        }
    }

    pub const fn enrichment_policy(&self) -> &EnrichmentPolicy {
        &self.enrichment_policy
    }

    pub const fn invalidated_cache_entries(&self) -> u32 {
        self.invalidated_cache_entries
    }
}

/// Durable, atomic metadata projection boundary. Implementations must re-read
/// access state inside the read/configure operation. Configuration must update
/// the profile policy and override mutations atomically and invalidate only
/// cache partitions affected by the changed policy.
pub trait MetadataProjectionPort: Send + Sync {
    fn authorize_and_read_projection(
        &self,
        query: ReadMetadataProjectionQuery,
    ) -> ApplicationResult<MetadataProjectionView>;

    fn authorize_and_configure_projection(
        &self,
        command: ConfigureMetadataProjectionCommand,
    ) -> ApplicationResult<ConfigureMetadataProjectionOutcome>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use fasti_domain::{ClientId, CredentialId, ProfileGrantId, ProfileId, WorkspaceId};

    fn access() -> RequestAccessContext {
        RequestAccessContext::new(
            WorkspaceId::new_v7(),
            ProfileId::new_v7(),
            ClientId::new_v7(),
            CredentialId::new_v7(),
            ProfileGrantId::new_v7(),
            1,
        )
    }

    #[test]
    fn provider_identity_coordinates_are_exact_and_shared_by_grain() {
        for (provider, kind, namespace, grain, pattern) in [
            (
                GOOGLE_BOOKS_PROVIDER_ID,
                "book",
                "googlebooks.volume",
                Grain::Edition,
                "[A-Za-z0-9_-]+",
            ),
            (
                TMDB_PROVIDER_ID,
                "movie",
                "tmdb.movie",
                Grain::Film,
                "^[1-9][0-9]*$",
            ),
            (
                TMDB_PROVIDER_ID,
                "show",
                "tmdb.tv",
                Grain::Series,
                "^[1-9][0-9]*$",
            ),
        ] {
            let by_kind = provider_identity_mapping(provider, kind).expect("mapped provider kind");
            let by_grain = provider_identity_mapping_for_grain(provider, grain)
                .expect("mapped provider grain");
            assert_eq!(by_kind, by_grain);
            assert_eq!(by_kind.namespace(), namespace);
            assert_eq!(by_kind.grain(), grain);
            let identifier = by_kind.identifier("42").expect("valid provider identifier");
            assert_eq!(identifier.namespace(), namespace);
            assert_eq!(identifier.grain(), grain);
            assert_eq!(
                by_kind
                    .namespace_definition()
                    .expect("provider namespace")
                    .id_pattern(),
                pattern
            );
        }

        assert!(provider_identity_mapping(GOOGLE_BOOKS_PROVIDER_ID, "chapter").is_none());
        assert!(provider_identity_mapping_for_grain(TMDB_PROVIDER_ID, Grain::Episode).is_none());
        assert!(provider_identity_mapping_for_grain(TMDB_PROVIDER_ID, Grain::Track).is_none());
        assert!(provider_identity_mapping(TMDB_PROVIDER_ID, "movie")
            .expect("TMDB movie mapping")
            .identifier("not-a-number")
            .is_err());
        for value in ["0", "00042", " 42 "] {
            assert!(provider_identity_mapping(TMDB_PROVIDER_ID, "movie")
                .expect("TMDB movie mapping")
                .identifier(value)
                .is_err());
        }
        assert!(provider_identity_mapping(GOOGLE_BOOKS_PROVIDER_ID, "book")
            .expect("Google Books mapping")
            .identifier("bad/value")
            .is_err());
    }

    fn identity_claim(namespace: &str, value: &str) -> ExternalIdentifierClaim {
        ExternalIdentifierClaim::try_new(namespace, Grain::Release, value)
            .expect("identity fixture")
    }

    fn identity_claim_at(namespace: &str, grain: Grain, value: &str) -> ExternalIdentifierClaim {
        ExternalIdentifierClaim::try_new(namespace, grain, value).expect("identity fixture")
    }

    #[test]
    fn tmdb_metadata_uses_the_pinned_nuvio_imdb_alias_without_rekeying() {
        let identifiers = vec![
            identity_claim("mal.anime", "49894"),
            identity_claim("imdb.title", "tt28254942"),
        ];
        let plan = plan_purpose_identity_route(
            ResolutionIntent::MetadataLookup,
            ProviderId::try_new("tmdb").expect("TMDB provider"),
            AnimeGroupingPreference::KeepMalReleasesSeparate,
            &identifiers,
        );

        assert_eq!(plan.status(), PurposeIdentityRouteStatus::Selected);
        assert_eq!(plan.known_identifiers().len(), identifiers.len());
        assert!(identifiers
            .iter()
            .all(|identifier| plan.known_identifiers().contains(identifier)));
        let route = plan.selected_route().expect("IMDb alias route");
        assert_eq!(route.identifier().namespace(), "imdb.title");
        assert_eq!(route.identifier().value(), "tt28254942");
        assert_eq!(route.kind(), IdentityRouteKind::VerifiedAlias);
    }

    #[test]
    fn tmdb_metadata_prefers_its_native_identifier_over_an_imdb_alias() {
        let tmdb = ExternalIdentifierClaim::try_new("tmdb.tv", Grain::Series, "42")
            .expect("TMDB identity fixture");
        let plan = plan_purpose_identity_route(
            ResolutionIntent::MetadataLookup,
            ProviderId::try_new("tmdb").expect("TMDB provider"),
            AnimeGroupingPreference::GroupByTvWork,
            &[tmdb.clone(), identity_claim("imdb.title", "tt28254942")],
        );

        let route = plan.selected_route().expect("native TMDB route");
        assert_eq!(route.identifier(), &tmdb);
        assert_eq!(route.kind(), IdentityRouteKind::ProviderNative);
    }

    #[test]
    fn tmdb_metadata_routes_aliases_before_an_accepted_crosswalk() {
        let imdb = IdentityRouteEvidence::direct(identity_claim_at(
            "imdb.title",
            Grain::Series,
            "tt0944947",
        ));
        let tvdb = IdentityRouteEvidence::direct(identity_claim_at(
            "tvdb.series",
            Grain::Series,
            "121361",
        ));
        let wikidata =
            IdentityRouteEvidence::direct(identity_claim_at("wikidata", Grain::Series, "Q23572"));
        let crosswalk = IdentityRouteEvidence::new(
            identity_claim_at("tmdb.tv", Grain::Series, "1399"),
            IdentityRouteEvidenceKind::AcceptedCrosswalk,
        );

        for (evidence, namespace, kind) in [
            (
                vec![imdb.clone(), tvdb.clone(), crosswalk.clone()],
                "imdb.title",
                IdentityRouteKind::VerifiedAlias,
            ),
            (
                vec![tvdb.clone(), crosswalk.clone()],
                "tvdb.series",
                IdentityRouteKind::VerifiedAlias,
            ),
            (
                vec![wikidata.clone(), crosswalk.clone()],
                "wikidata",
                IdentityRouteKind::VerifiedAlias,
            ),
            (
                vec![crosswalk.clone()],
                "tmdb.tv",
                IdentityRouteKind::AcceptedCrosswalk,
            ),
        ] {
            let plan = plan_purpose_identity_route_with_evidence(
                ResolutionIntent::MetadataEnrichment,
                ProviderId::try_new("tmdb").expect("TMDB provider"),
                AnimeGroupingPreference::Automatic,
                &evidence,
            );
            let route = plan.selected_route().expect("selected TMDB route");
            assert_eq!(route.identifier().namespace(), namespace);
            assert_eq!(route.kind(), kind);
        }
    }

    #[test]
    fn tracker_write_uses_only_the_target_provider_native_identifier() {
        let plan = plan_purpose_identity_route(
            ResolutionIntent::TrackerWrite,
            ProviderId::try_new("kitsu").expect("Kitsu provider"),
            AnimeGroupingPreference::GroupByTvWork,
            &[
                identity_claim("imdb.title", "tt28254942"),
                identity_claim("mal.anime", "49894"),
                identity_claim("kitsu.anime", "7442"),
            ],
        );

        let route = plan.selected_route().expect("Kitsu write route");
        assert_eq!(route.identifier().namespace(), "kitsu.anime");
        assert_eq!(route.kind(), IdentityRouteKind::ProviderNative);
        assert_eq!(plan.candidate_routes().len(), 1);
    }

    #[test]
    fn tracker_write_fails_closed_without_the_target_provider_identifier() {
        let plan = plan_purpose_identity_route(
            ResolutionIntent::TrackerWrite,
            ProviderId::try_new("kitsu").expect("Kitsu provider"),
            AnimeGroupingPreference::GroupByTvWork,
            &[
                identity_claim("imdb.title", "tt28254942"),
                identity_claim("mal.anime", "49894"),
            ],
        );

        assert_eq!(plan.status(), PurposeIdentityRouteStatus::Missing);
        assert!(plan.selected_route().is_none());
        assert!(plan.candidate_routes().is_empty());
    }

    #[test]
    fn equally_ranked_aliases_fail_closed_as_ambiguous() {
        let plan = plan_purpose_identity_route(
            ResolutionIntent::MetadataLookup,
            ProviderId::try_new("tmdb").expect("TMDB provider"),
            AnimeGroupingPreference::GroupByTvWork,
            &[
                identity_claim("imdb.title", "tt0000001"),
                identity_claim("imdb.title", "tt0000002"),
            ],
        );

        assert_eq!(plan.status(), PurposeIdentityRouteStatus::Ambiguous);
        assert!(plan.selected_route().is_none());
        assert_eq!(plan.candidate_routes().len(), 2);
    }

    #[test]
    fn anime_export_preference_changes_only_the_selected_projection() {
        let identifiers = [
            identity_claim("imdb.title", "tt28254942"),
            identity_claim("mal.anime", "49894"),
            identity_claim("kitsu.anime", "7442"),
        ];
        for (preference, namespace) in [
            (AnimeGroupingPreference::GroupByTvWork, "imdb.title"),
            (AnimeGroupingPreference::Automatic, "imdb.title"),
            (
                AnimeGroupingPreference::KeepMalReleasesSeparate,
                "mal.anime",
            ),
            (
                AnimeGroupingPreference::KeepKitsuReleasesSeparate,
                "kitsu.anime",
            ),
        ] {
            let plan = plan_purpose_identity_route(
                ResolutionIntent::NuvioExport,
                ProviderId::try_new("nuvio").expect("Nuvio provider"),
                preference,
                &identifiers,
            );
            assert_eq!(
                plan.selected_route()
                    .expect("selected anime export route")
                    .identifier()
                    .namespace(),
                namespace
            );
            assert_eq!(plan.known_identifiers().len(), identifiers.len());
            assert!(identifiers
                .iter()
                .all(|identifier| plan.known_identifiers().contains(identifier)));
        }
    }

    #[test]
    fn tv_work_grouping_accepts_a_series_grained_coordinate() {
        let plan = plan_purpose_identity_route(
            ResolutionIntent::NuvioExport,
            ProviderId::try_new("nuvio").expect("Nuvio provider"),
            AnimeGroupingPreference::GroupByTvWork,
            &[
                identity_claim("mal.anime", "49894"),
                identity_claim_at("tmdb.tv", Grain::Series, "1399"),
            ],
        );

        let route = plan.selected_route().expect("series grouping route");
        assert_eq!(route.identifier().namespace(), "tmdb.tv");
        assert_eq!(route.identifier().grain(), Grain::Series);
    }

    #[test]
    fn nuvio_tv_work_grouping_uses_its_pinned_imdb_first_order() {
        let plan = plan_purpose_identity_route(
            ResolutionIntent::NuvioExport,
            ProviderId::try_new("nuvio").expect("Nuvio provider"),
            AnimeGroupingPreference::Automatic,
            &[
                identity_claim_at("tvdb.series", Grain::Series, "121361"),
                identity_claim_at("tmdb.tv", Grain::Series, "1399"),
                identity_claim("imdb.title", "tt0944947"),
            ],
        );

        assert_eq!(plan.status(), PurposeIdentityRouteStatus::Selected);
        assert_eq!(
            plan.selected_route()
                .expect("Nuvio-compatible TV work route")
                .identifier()
                .namespace(),
            "imdb.title"
        );
    }

    #[test]
    fn nuvio_release_grouping_keeps_its_pinned_anime_fallback_order() {
        let identifiers = [
            identity_claim("imdb.title", "tt5311514"),
            identity_claim("anidb.anime", "15159"),
            identity_claim("anilist.anime", "32281"),
            identity_claim("kitsu.anime", "12268"),
            identity_claim("simkl.anime", "60001"),
        ];
        let nuvio = ProviderId::try_new("nuvio").expect("Nuvio provider");

        let mal = plan_purpose_identity_route(
            ResolutionIntent::NuvioExport,
            nuvio.clone(),
            AnimeGroupingPreference::KeepMalReleasesSeparate,
            &identifiers,
        );
        assert_eq!(
            mal.selected_route()
                .expect("MAL-compatible fallback")
                .identifier()
                .namespace(),
            "kitsu.anime"
        );

        let kitsu = plan_purpose_identity_route(
            ResolutionIntent::NuvioExport,
            nuvio,
            AnimeGroupingPreference::KeepKitsuReleasesSeparate,
            &identifiers[0..3],
        );
        assert_eq!(
            kitsu
                .selected_route()
                .expect("Kitsu-compatible fallback")
                .identifier()
                .namespace(),
            "anidb.anime"
        );
    }

    #[test]
    fn nuvio_content_ids_use_one_canonical_wire_encoder() {
        for (namespace, grain, value, expected) in [
            ("imdb.title", Grain::Series, "tt0944947", "tt0944947"),
            ("tmdb.tv", Grain::Series, "1399", "tmdb:1399"),
            ("tvdb.series", Grain::Series, "121361", "tvdb:121361"),
            ("mal.anime", Grain::Release, "49894", "mal:49894"),
            ("anidb.anime", Grain::Release, "15159", "anidb:15159"),
            ("anilist.anime", Grain::Release, "32281", "anilist:32281"),
            ("kitsu.anime", Grain::Release, "12268", "kitsu:12268"),
            ("simkl.anime", Grain::Release, "60001", "simkl:60001"),
        ] {
            let plan = plan_purpose_identity_route(
                ResolutionIntent::NuvioExport,
                ProviderId::try_new("nuvio").expect("Nuvio provider"),
                AnimeGroupingPreference::Automatic,
                &[identity_claim_at(namespace, grain, value)],
            );
            assert_eq!(plan.nuvio_content_id().as_deref(), Some(expected));
        }

        let metadata_plan = plan_purpose_identity_route(
            ResolutionIntent::MetadataLookup,
            ProviderId::try_new("tmdb").expect("TMDB provider"),
            AnimeGroupingPreference::Automatic,
            &[identity_claim("imdb.title", "tt28254942")],
        );
        assert!(metadata_plan.nuvio_content_id().is_none());
    }

    #[test]
    fn anime_grouping_preview_reports_change_and_possible_regrouping() {
        let preview = preview_anime_grouping_change_for_record(
            RecordId::new_v7(),
            AnimeGroupingPreference::GroupByTvWork,
            AnimeGroupingPreference::KeepMalReleasesSeparate,
            &[
                identity_claim("imdb.title", "tt28254942"),
                identity_claim("mal.anime", "49894"),
            ],
        );

        assert!(preview.route_changed());
        assert!(!preview.unresolved());
        assert!(preview.possible_season_regrouping());
        assert_eq!(
            preview
                .previous_route()
                .expect("previous route")
                .identifier()
                .namespace(),
            "imdb.title"
        );
        assert_eq!(
            preview
                .proposed_route()
                .expect("proposed route")
                .identifier()
                .namespace(),
            "mal.anime"
        );
    }

    #[test]
    fn anime_grouping_preview_preserves_safe_missing_state() {
        let record_id = RecordId::new_v7();
        let preview = preview_anime_grouping_change_for_record(
            record_id,
            AnimeGroupingPreference::GroupByTvWork,
            AnimeGroupingPreference::KeepKitsuReleasesSeparate,
            &[identity_claim("local.unmapped", "17723")],
        );

        assert_eq!(preview.record_id(), record_id);
        assert!(!preview.route_changed());
        assert!(preview.unresolved());
        assert!(!preview.possible_season_regrouping());
        assert_eq!(
            preview.previous_status(),
            PurposeIdentityRouteStatus::Missing
        );
        assert_eq!(
            preview.proposed_status(),
            PurposeIdentityRouteStatus::Missing
        );
        assert!(preview.previous_route().is_none());
        assert!(preview.proposed_route().is_none());
    }

    #[test]
    fn refresh_command_canonicalizes_field_groups_without_client_cache_coordinates() {
        let command = RefreshMetadataClaimsCommand::new(
            RequestCorrelationId::new_v7(),
            access(),
            fasti_domain::OperationId::new_v7(),
            RecordId::new_v7(),
            MetadataProviderId::try_new("tmdb").expect("provider"),
            vec![
                MetadataFieldGroup::Details,
                MetadataFieldGroup::BasicInfo,
                MetadataFieldGroup::Details,
            ],
            Some(MetadataLocale::try_new("en-IE").expect("locale")),
            Some(MetadataRegion::try_new("ie").expect("region")),
            MetadataRefreshMode::Revalidate,
        );
        assert_eq!(
            command.field_groups(),
            &[MetadataFieldGroup::BasicInfo, MetadataFieldGroup::Details]
        );
        assert_eq!(command.locale().map(MetadataLocale::as_str), Some("en-ie"));
        assert_eq!(command.region().map(MetadataRegion::as_str), Some("IE"));
    }

    #[test]
    fn projection_configuration_is_bound_to_authenticated_profile() {
        let access = access();
        let profile_id = access.profile_id();
        let command = ConfigureMetadataProjectionCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            MetadataProjectionPolicy::default_for_profile(profile_id),
            None,
            vec![MetadataFieldGroup::Artwork],
            vec![],
        );
        assert_eq!(command.access().profile_id(), profile_id);
        assert_eq!(command.projection_policy().profile_id(), profile_id);
    }
}
