//! Provider metadata application commands.
//!
//! Provider adapters fetch outside the local transaction, then hand validated
//! claims to this port. The provider coordinate remains evidence attached to a
//! Fasti Record; it never becomes the Record identity.

use crate::ProviderCapabilityState;
use crate::{ApplicationResult, RequestAccessContext};
use fasti_domain::{
    EnrichmentPolicy, ExternalIdentifierClaim, ExternalIdentifierError, FieldClaim,
    FieldClaimStatus, FieldKey, Grain, MetadataAttribution, MetadataCacheEntry,
    MetadataCacheReadState, MetadataFieldGroup, MetadataLocale, MetadataProjection,
    MetadataProjectionPolicy, MetadataProviderId, MetadataRegion, NamespaceDefinition,
    NamespaceDefinitionError, NamespaceLicencePosture, ProfileId, RatingClaim, RecordId,
    RequestCorrelationId, MAX_EXTERNAL_IDENTIFIER_BYTES, ORIGINAL_TITLE_FIELD_KEY,
    OVERVIEW_FIELD_KEY, POSTER_FIELD_KEY, RELEASE_YEAR_FIELD_KEY, TITLE_FIELD_KEY,
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

impl CommitMetadataRefreshCommand {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
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

    #[test]
    fn refresh_command_canonicalizes_field_groups_without_client_cache_coordinates() {
        let command = RefreshMetadataClaimsCommand::new(
            RequestCorrelationId::new_v7(),
            access(),
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
