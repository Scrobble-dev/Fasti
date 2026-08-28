use crate::records::require_access;
use crate::setup::{DesktopProblem, SetupSecretStore};
use fasti_application::{
    ClearNuvioCollectionsCommand, GetNuvioCollectionsQuery, NuvioCollectionsPort,
    ReplaceNuvioCollectionsCommand,
};
use fasti_contracts::{NuvioCollectionsDocumentDto, NuvioCollectionsStateDto};
use fasti_domain::RequestCorrelationId;
use fasti_store::SqliteKernel;

fn state(
    document: Option<&fasti_application::NuvioCollectionsDocument>,
) -> NuvioCollectionsStateDto {
    NuvioCollectionsStateDto {
        document: document.map(NuvioCollectionsDocumentDto::from_application),
    }
}

pub(crate) fn get(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
) -> Result<NuvioCollectionsStateDto, DesktopProblem> {
    let access = require_access(kernel, store)?;
    let document = kernel
        .get_nuvio_collections(GetNuvioCollectionsQuery::new(
            RequestCorrelationId::new_v7(),
            access,
        ))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(state(document.as_ref()))
}

pub(crate) fn replace(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
    document: NuvioCollectionsDocumentDto,
) -> Result<NuvioCollectionsStateDto, DesktopProblem> {
    let document = document.into_application().map_err(|error| {
        DesktopProblem::invalid_input(format!(
            "Invalid Nuvio Collections document: {}",
            error.reason()
        ))
    })?;
    let access = require_access(kernel, store)?;
    let document = kernel
        .replace_nuvio_collections(ReplaceNuvioCollectionsCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            document,
        ))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(state(Some(&document)))
}

pub(crate) fn clear(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
) -> Result<NuvioCollectionsStateDto, DesktopProblem> {
    let access = require_access(kernel, store)?;
    kernel
        .clear_nuvio_collections(ClearNuvioCollectionsCommand::new(
            RequestCorrelationId::new_v7(),
            access,
        ))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(state(None))
}
