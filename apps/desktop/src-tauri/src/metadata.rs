use crate::setup::DesktopProblem;
use fasti_application::{
    ConfigureMetadataProjectionCommand, MetadataClaimRefreshService, MetadataOverrideMutation,
    MetadataProjectionPort, ReadMetadataProjectionQuery, RefreshMetadataClaimsCommand,
    RequestAccessContext,
};
use fasti_contracts::{
    metadata_field_group, metadata_projection_configuration_response,
    metadata_projection_response, metadata_refresh_mode, refresh_metadata_claims_response,
    ConfigureMetadataProjectionRequest, LastKnownGoodPolicyDto, MetadataFieldGroupDto,
    MetadataProjectionConfigurationResponse,
    MetadataProjectionResponse, RefreshMetadataClaimsRequest, RefreshMetadataClaimsResponse,
};
use fasti_domain::{
    FieldKey, LastKnownGoodPolicy, MetadataLocale, MetadataProjectionPolicy, MetadataProviderId,
    MetadataRegion, RecordId, RequestCorrelationId, MAX_FIELD_VALUE_BYTES,
};
use fasti_provider_runtime::{ProviderMetadataRefreshService, ProviderRuntime};
use fasti_store::SqliteKernel;
use serde::Deserialize;
use std::{collections::HashSet, sync::Arc};

const MAX_FIELD_GROUPS: usize = 32;
const MAX_OVERRIDES: usize = 64;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadMetadataProjectionInput {
    record_id: String,
    offline: bool,
}

fn field_groups(
    values: Vec<MetadataFieldGroupDto>,
    allow_empty: bool,
) -> Result<Vec<fasti_domain::MetadataFieldGroup>, DesktopProblem> {
    if (!allow_empty && values.is_empty()) || values.len() > MAX_FIELD_GROUPS {
        return Err(DesktopProblem::invalid_input(
            "metadata field groups must contain a bounded list",
        ));
    }
    let groups = values
        .into_iter()
        .map(metadata_field_group)
        .collect::<Vec<_>>();
    if groups.iter().copied().collect::<HashSet<_>>().len() != groups.len() {
        return Err(DesktopProblem::invalid_input(
            "metadata field groups must be unique",
        ));
    }
    Ok(groups)
}

pub(crate) async fn refresh(
    kernel: Arc<SqliteKernel>,
    runtime: Arc<ProviderRuntime>,
    policy: fasti_application::OutboundAccessPolicy,
    access: RequestAccessContext,
    request: RefreshMetadataClaimsRequest,
) -> Result<RefreshMetadataClaimsResponse, DesktopProblem> {
    let correlation_id = RequestCorrelationId::new_v7();
    let record_id = request
        .record_id
        .parse::<RecordId>()
        .map_err(|_| DesktopProblem::invalid_input("record_id is not a valid record identifier"))?;
    let provider_id = MetadataProviderId::try_new(request.provider_id)
        .map_err(|_| DesktopProblem::invalid_input("provider_id is not valid"))?;
    let groups = field_groups(request.field_groups, false)?;
    let locale = request
        .locale
        .map(MetadataLocale::try_new)
        .transpose()
        .map_err(|_| DesktopProblem::invalid_input("locale is not valid"))?;
    let region = request
        .region
        .map(MetadataRegion::try_new)
        .transpose()
        .map_err(|_| DesktopProblem::invalid_input("region is not valid"))?;
    let service = ProviderMetadataRefreshService::new(
        runtime,
        Arc::clone(&kernel) as Arc<dyn fasti_application::MetadataRefreshPersistencePort>,
        kernel as Arc<dyn fasti_application::ProviderStatePort>,
        policy,
    );
    let provider_id_text = provider_id.as_str().to_owned();
    let outcome = service
        .authorize_and_refresh(RefreshMetadataClaimsCommand::new(
            correlation_id,
            access,
            record_id,
            provider_id,
            groups,
            locale,
            region,
            metadata_refresh_mode(request.mode),
        ))
        .await
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(refresh_metadata_claims_response(
        record_id,
        &provider_id_text,
        &outcome,
    ))
}

pub(crate) fn read(
    kernel: &SqliteKernel,
    access: RequestAccessContext,
    input: ReadMetadataProjectionInput,
) -> Result<MetadataProjectionResponse, DesktopProblem> {
    let record_id = input
        .record_id
        .parse::<RecordId>()
        .map_err(|_| DesktopProblem::invalid_input("record_id is not a valid record identifier"))?;
    let view = kernel
        .authorize_and_read_projection(ReadMetadataProjectionQuery::new(
            RequestCorrelationId::new_v7(),
            access,
            record_id,
            input.offline,
        ))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(metadata_projection_response(&view))
}

pub(crate) fn configure(
    kernel: &SqliteKernel,
    access: RequestAccessContext,
    request: ConfigureMetadataProjectionRequest,
) -> Result<MetadataProjectionConfigurationResponse, DesktopProblem> {
    let preferred_provider_id = request
        .preferred_provider_id
        .map(MetadataProviderId::try_new)
        .transpose()
        .map_err(|_| DesktopProblem::invalid_input("preferred_provider_id is not valid"))?;
    let preferred_locale = request
        .preferred_locale
        .map(MetadataLocale::try_new)
        .transpose()
        .map_err(|_| DesktopProblem::invalid_input("preferred_locale is not valid"))?;
    let original_locale = request
        .original_locale
        .map(MetadataLocale::try_new)
        .transpose()
        .map_err(|_| DesktopProblem::invalid_input("original_locale is not valid"))?;
    let region = request
        .region
        .map(MetadataRegion::try_new)
        .transpose()
        .map_err(|_| DesktopProblem::invalid_input("region is not valid"))?;
    let groups = field_groups(request.enabled_field_groups, true)?;
    if request.overrides.len() > MAX_OVERRIDES {
        return Err(DesktopProblem::invalid_input(
            "at most 64 metadata overrides are permitted",
        ));
    }
    let mut targets = HashSet::with_capacity(request.overrides.len());
    let mut overrides = Vec::with_capacity(request.overrides.len());
    for value in request.overrides {
        let (record_text, field_text, set_value) = match value {
            fasti_contracts::MetadataOverrideMutationDto::Set {
                record_id,
                field_key,
                value,
            } => (record_id, field_key, Some(value)),
            fasti_contracts::MetadataOverrideMutationDto::Clear {
                record_id,
                field_key,
            } => (record_id, field_key, None),
        };
        let record_id = record_text.parse::<RecordId>().map_err(|_| {
            DesktopProblem::invalid_input("override record_id is not a valid record identifier")
        })?;
        let field_key = FieldKey::try_new(field_text)
            .map_err(|_| DesktopProblem::invalid_input("override field_key is not valid"))?;
        if !targets.insert((record_id, field_key.clone())) {
            return Err(DesktopProblem::invalid_input(
                "metadata override targets must be unique",
            ));
        }
        let mutation = match set_value {
            Some(value)
                if !value.is_empty()
                    && value.len() <= MAX_FIELD_VALUE_BYTES
                    && !value.chars().any(char::is_control) =>
            {
                MetadataOverrideMutation::Set {
                    record_id,
                    field_key,
                    value,
                }
            }
            None => MetadataOverrideMutation::Clear {
                record_id,
                field_key,
            },
            Some(_) => {
                return Err(DesktopProblem::invalid_input(
                    "metadata override operation and value do not match",
                ));
            }
        };
        overrides.push(mutation);
    }
    let profile_id = access.profile_id();
    let policy = MetadataProjectionPolicy::new(
        profile_id,
        preferred_provider_id,
        preferred_locale,
        original_locale,
        request.allow_english_fallback,
        match request.last_known_good {
            LastKnownGoodPolicyDto::Allow => LastKnownGoodPolicy::Allow,
            LastKnownGoodPolicyDto::Deny => LastKnownGoodPolicy::Deny,
        },
    );
    let outcome = kernel
        .authorize_and_configure_projection(ConfigureMetadataProjectionCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            policy,
            region,
            groups,
            overrides,
        ))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(metadata_projection_configuration_response(&outcome))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_field_groups_are_rejected_before_domain_deduplication() {
        assert!(field_groups(
            vec![
                MetadataFieldGroupDto::BasicInfo,
                MetadataFieldGroupDto::BasicInfo,
            ],
            false,
        )
        .is_err());
    }
}
