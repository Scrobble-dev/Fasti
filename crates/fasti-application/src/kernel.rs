//! B2 local-kernel commands, queries, outcomes, and adapter ports.

use crate::{ApplicationResult, RequestAccessContext};
use fasti_domain::{
    EvidenceReference, ExternalIdentifierClaim, ExternalIdentifierId, Grain, InterpretationId,
    ProfileId, RecordId, ReviewItemId, ReviewStatus, WorkspaceId,
};
use std::fmt;

const SECRET_BYTES: usize = 32;
const SECRET_HEX_BYTES: usize = SECRET_BYTES * 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretParseError;

impl fmt::Display for SecretParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("secret must contain exactly 64 lowercase hexadecimal characters")
    }
}

impl std::error::Error for SecretParseError {}

/// One-time secret material.
///
/// This type deliberately implements neither `Debug`, `Clone`, nor
/// serialization. Delivery adapters may expose it only in the one successful
/// response that creates or rotates the credential.
pub struct SecretMaterial {
    bytes: [u8; SECRET_BYTES],
}

impl SecretMaterial {
    pub fn from_bytes(bytes: [u8; SECRET_BYTES]) -> Self {
        Self { bytes }
    }

    pub fn try_from_hex(value: &str) -> Result<Self, SecretParseError> {
        if value.len() != SECRET_HEX_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(SecretParseError);
        }
        let mut bytes = [0_u8; SECRET_BYTES];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (decode_hex(pair[0])? << 4) | decode_hex(pair[1])?;
        }
        Ok(Self { bytes })
    }

    pub fn expose_hex(&self) -> String {
        let mut value = String::with_capacity(SECRET_HEX_BYTES);
        for byte in self.bytes {
            use std::fmt::Write as _;
            write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
        }
        value
    }

    pub fn expose_bytes(&self) -> &[u8; SECRET_BYTES] {
        &self.bytes
    }
}

impl Drop for SecretMaterial {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}

fn decode_hex(value: u8) -> Result<u8, SecretParseError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(SecretParseError),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitializeNodeCommand {
    correlation_id: fasti_domain::RequestCorrelationId,
}

impl InitializeNodeCommand {
    pub const fn new(correlation_id: fasti_domain::RequestCorrelationId) -> Self {
        Self { correlation_id }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }
}

pub struct InitializeNodeOutcome {
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    client_id: fasti_domain::ClientId,
    initialization_proof: SecretMaterial,
}

impl InitializeNodeOutcome {
    pub fn new(
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
        client_id: fasti_domain::ClientId,
        initialization_proof: SecretMaterial,
    ) -> Self {
        Self {
            workspace_id,
            profile_id,
            client_id,
            initialization_proof,
        }
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub const fn client_id(&self) -> fasti_domain::ClientId {
        self.client_id
    }

    pub const fn initialization_proof(&self) -> &SecretMaterial {
        &self.initialization_proof
    }
}

pub struct EnrollFirstClientCommand {
    correlation_id: fasti_domain::RequestCorrelationId,
    initialization_proof: SecretMaterial,
}

impl EnrollFirstClientCommand {
    pub const fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        initialization_proof: SecretMaterial,
    ) -> Self {
        Self {
            correlation_id,
            initialization_proof,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn initialization_proof(&self) -> &SecretMaterial {
        &self.initialization_proof
    }
}

pub struct EnrollFirstClientOutcome {
    access: RequestAccessContext,
    credential: SecretMaterial,
}

impl EnrollFirstClientOutcome {
    pub const fn new(access: RequestAccessContext, credential: SecretMaterial) -> Self {
        Self { access, credential }
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn credential(&self) -> &SecretMaterial {
        &self.credential
    }
}

pub struct AuthenticateCredentialQuery {
    correlation_id: fasti_domain::RequestCorrelationId,
    credential: SecretMaterial,
    profile_id: ProfileId,
}

impl AuthenticateCredentialQuery {
    pub const fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        credential: SecretMaterial,
        profile_id: ProfileId,
    ) -> Self {
        Self {
            correlation_id,
            credential,
            profile_id,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn credential(&self) -> &SecretMaterial {
        &self.credential
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotateCredentialCommand {
    correlation_id: fasti_domain::RequestCorrelationId,
    access: RequestAccessContext,
}

impl RotateCredentialCommand {
    pub const fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        access: RequestAccessContext,
    ) -> Self {
        Self {
            correlation_id,
            access,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
}

pub struct RotateCredentialOutcome {
    access: RequestAccessContext,
    credential: SecretMaterial,
}

impl RotateCredentialOutcome {
    pub const fn new(access: RequestAccessContext, credential: SecretMaterial) -> Self {
        Self { access, credential }
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn credential(&self) -> &SecretMaterial {
        &self.credential
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevokeCredentialCommand {
    correlation_id: fasti_domain::RequestCorrelationId,
    access: RequestAccessContext,
    target_credential_id: fasti_domain::CredentialId,
}

impl RevokeCredentialCommand {
    pub const fn new(
        correlation_id: fasti_domain::RequestCorrelationId,
        access: RequestAccessContext,
        target_credential_id: fasti_domain::CredentialId,
    ) -> Self {
        Self {
            correlation_id,
            access,
            target_credential_id,
        }
    }

    pub const fn correlation_id(&self) -> fasti_domain::RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn target_credential_id(&self) -> fasti_domain::CredentialId {
        self.target_credential_id
    }
}

#[derive(Debug, Cl²È="25Ù”¡•‰Õœ°±½¹”°½Áä°A…ÉÑ¥…±Ä°Ä¥t)ÁÕˆ•¹Õ´I•Ù¥•ÝI•Í½±ÕÑ¥½¹Q…É•Ðì(€€€á¥ÍÑ¥¹œ¡I•½É‘%¤°(€€€9•Ü¡É…¥¸¤°)ô((m‘•É¥Ù”¡•‰Õœ°±½¹”°A…ÉÑ¥…±Ä°Ä¥t)ÁÕˆÍÑÉÕÐI•Í½±Ù•I•Ù¥•Ý½µµ…¹ì(€€€½ÉÉ•±…Ñ¥½¹}¥è™…ÍÑ¥}‘½µ…¥¸èéI•ÅÕ•ÍÑ½ÉÉ•±…Ñ¥½¹%°(€€€…•ÍÌèI•ÅÕ•ÍÑ•ÍÍ½¹Ñ•áÐ°(€€€É•Ù¥•Ý}¥Ñ•µ}¥èI•Ù¥•Ý%Ñ•µ%°(€€€Ñ…É•ÐèI•Ù¥•ÝI•Í½±ÕÑ¥½¹Q…É•Ð°(€€€¥‘•¹Ñ¥™¥•ÉÌèY•ŒñáÑ•É¹…±%‘•¹Ñ¥™¥•É±…¥´ø°)ô()¥µÁ°I•Í½±Ù•I•Ù¥•Ý½µµ…¹ì(€€€ÁÕˆ™¸¹•Ü (€€€€€€€½ÉÉ•±…Ñ¥½¹}¥è™…ÍÑ¥}‘½µ…¥¸èéI•ÅÕ•ÍÑ½ÉÉ•±…Ñ¥½¹%°(€€€€€€€…•ÍÌèI•ÅÕ•ÍÑ•ÍÍ½¹Ñ•áÐ°(€€€€€€€É•Ù¥•Ý}¥Ñ•µ}¥èI•Ù¥•Ý%Ñ•µ%°(€€€€€€€Ñ…É•ÐèI•Ù¥•ÝI•Í½±ÕÑ¥½¹Q…É•Ð°(€€€€€€€¥‘•¹Ñ¥™¥•ÉÌèY•ŒñáÑ•É¹…±%‘•¹Ñ¥™¥•É±…¥´ø°(€€€€¤€´øM•±˜ì(€€€€€€€M•±˜ì(€€€€€€€€€€€½ÉÉ•±…Ñ¥½¹}¥°(€€€€€€€€€€€…•ÍÌ°(€€€€€€€€€€€É•Ù¥•Ý}¥Ñ•µ}¥°(€€€€€€€€€€€Ñ…É•Ð°(€€€€€€€€€€€¥‘•¹Ñ¥™¥•ÉÌì(€€€€€€€ô(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸½ÉÉ•±…Ñ¥½¹}¥ ™Í•±˜¤€´ø™…ÍÑ¥}‘½µ…¥¸èéI•ÅÕ•ÍÑ½ÉÉ•±…Ñ¥½¹%ì(€€€€€€€Í•±˜¹½ÉÉ•±…Ñ¥½¹}¥(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸…•ÍÌ ™Í•±˜¤€´ø€™I•ÅÕ•ÍÑ•ÍÍ½¹Ñ•áÐì(€€€€€€€€™Í•±˜¹…•ÍÌ(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸É•Ù¥•Ý}¥Ñ•µ}¥ ™Í•±˜¤€´øI•Ù¥•Ý%Ñ•µ%ì(€€€€€€€Í•±˜¹É•Ù¥•Ý}¥Ñ•µ}¥(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸Ñ…É•Ð ™Í•±˜¤€´øI•Ù¥•ÝI•Í½±ÕÑ¥½¹Q…É•Ðì(€€€€€€€Í•±˜¹Ñ…É•Ð(€€€ô((€€€ÁÕˆ™¸¥‘•¹Ñ¥™¥•ÉÌ ™Í•±˜¤€´ø€™máÑ•É¹…±%‘•¹Ñ¥™¥•É±…¥µtì(€€€€€€€€™Í•±˜¹¥‘•¹Ñ¥™¥•ÉÌ(€€€ô)ô((m‘•É¥Ù”¡•‰Õœ°±½¹”°½Áä°A…ÉÑ¥…±Ä°Ä¥t)ÁÕˆÍÑÉÕÐI•Í½±Ù•I•Ù¥•Ý=ÕÑ½µ”ì(€€€É•Ù¥•Ý}¥Ñ•µ}¥èI•Ù¥•Ý%Ñ•µ%°(€€€É•½É‘}¥èI•½É‘%°(€€€¥¹Ñ•ÉÁÉ•Ñ…Ñ¥½¹}¥è%¹Ñ•ÉÁÉ•Ñ…Ñ¥½¹%°)ô()¥µÁ°I•Í½±Ù•I•Ù¥•Ý=ÕÑ½µ”ì(€€€ÁÕˆ½¹ÍÐ™¸¹•Ü (€€€€€€€É•Ù¥•Ý}¥Ñ•µ}¥èI•Ù¥•Ý%Ñ•µ%°(€€€€€€€É•½É‘}¥èI•½É‘%°(€€€€€€€¥¹Ñ•ÉÁÉ•Ñ…Ñ¥½¹}¥è%¹Ñ•ÉÁÉ•Ñ…Ñ¥½¹%°(€€€€¤€´øM•±˜ì(€€€€€€€M•±˜ì(€€€€€€€€€€€É•Ù¥•Ý}¥Ñ•µ}¥°(€€€€€€€€€€€É•½É‘}¥°(€€€€€€€€€€€¥¹Ñ•ÉÁÉ•Ñ…Ñ¥½¹}¥°(€€€€€€€ô(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸É•Ù¥•Ý}¥Ñ•µ}¥ ™Í•±˜¤€´øI•Ù¥•Ý%Ñ•µ%ì(€€€€€€€Í•±˜¹É•Ù¥•Ý}¥Ñ•µ}¥(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸É•½É‘}¥ ™Í•±˜¤€´øI•½É‘%ì(€€€€€€€Í•±˜¹É•½É‘}¥(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸¥¹Ñ•ÉÁÉ•Ñ…Ñ¥½¹}¥ ™Í•±˜¤€´ø%¹Ñ•ÉÁÉ•Ñ…Ñ¥½¹%ì(€€€€€€€Í•±˜¹¥¹Ñ•ÉÁÉ•Ñ…Ñ¥½¹}¥(€€€ô)ô()ÁÕˆÑÉ…¥ÐI•Ù¥•ÝA½ÉÐèM•¹€¬Må¹Œì(€€€™¸¥¹ÍÁ•Ñ}É•Ù¥•ÝÌ ™Í•±˜°ÅÕ•ÉäèI•Ù¥•ÝEÕ•Éä¤€´øÁÁ±¥…Ñ¥½¹I•ÍÕ±ÐñY•ŒñI•Ù¥•Ý%Ñ•µY¥•Üøøì((€€€™¸¡…¹•}É•Ù¥•Ý}ÍÑ…ÑÕÌ (€€€€€€€€™Í•±˜°(€€€€€€€½µµ…¹èI•Ù¥•ÝÑ¥½¹½µµ…¹°(€€€€¤€´øÁÁ±¥…Ñ¥½¹I•ÍÕ±ÐñI•Ù¥•Ý%Ñ•µY¥•Üøì((€€€™¸É•Í½±Ù•}É•Ù¥•Ü (€€€€€€€€™Í•±˜°(€€€€€€€½µµ…¹èI•Í½±Ù•I•Ù¥•Ý½µµ…¹°(€€€€¤€´øÁÁ±¥…Ñ¥½¹I•ÍÕ±ÐñI•Í½±Ù•I•Ù¥•Ý=ÕÑ½µ”øì)ô((m‘•É¥Ù”¡•‰Õœ°±½¹”°A…ÉÑ¥…±Ä°Ä¥t)ÁÕˆÍÑÉÕÐ%‘•¹Ñ¥ÑåM••‘¹ÑÉäì(€€€­•äèMÑÉ¥¹œ°(€€€É…¥¸èÉ…¥¸°(€€€¥‘•¹Ñ¥™¥•ÉÌèY•ŒñáÑ•É¹…±%‘•¹Ñ¥™¥•É±…¥´ø°)ô()¥µÁ°%‘•¹Ñ¥ÑåM••‘¹ÑÉäì(€€€ÁÕˆ™¸¹•Ü (€€€€€€€­•äè¥µÁ°%¹Ñ¼ñMÑÉ¥¹œø°(€€€€€€€É…¥¸èÉ…¥¸°(€€€€€€€¥‘•¹Ñ¥™¥•ÉÌèY•ŒñáÑ•É¹…±%‘•¹Ñ¥™¥•É±…¥´ø°(€€€€¤€´øM•±˜ì(€€€€€€€M•±˜ì(€€€€€€€€€€€­•äè­•ä¹¥¹Ñ¼ ¤°(€€€€€€€€€€€É…¥¸°(€€€€€€€€€€€¥‘•¹Ñ¥™¥•ÉÌ°(€€€€€€€ô(€€€ô((€€€ÁÕˆ™¸­•ä ™Í•±˜¤€´ø€™ÍÑÈì(€€€€€€€€™Í•±˜¹­•ä(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸É…¥¸ ™Í•±˜¤€´øÉ…¥¸ì(€€€€€€€Í•±˜¹É…¥¸(€€€ô((€€€ÁÕˆ™¸¥‘•¹Ñ¥™¥•ÉÌ ™Í•±˜¤€´ø€™máÑ•É¹…±%‘•¹Ñ¥™¥•É±…¥µtì(€€€€€€€€™Í•±˜¹¥‘•¹Ñ¥™¥•ÉÌ(€€€ô)ô((m‘•É¥Ù”¡•‰Õœ°±½¹”°A…ÉÑ¥…±Ä°Ä¥t)ÁÕˆÍÑÉÕÐ%‘•¹Ñ¥ÑåM••‘5…¹¥™•ÍÐì(€€€Ù•ÉÍ¥½¸èMÑÉ¥¹œ°(€€€•¹ÑÉ¥•ÌèY•Œñ%‘•¹Ñ¥ÑåM••‘¹ÑÉäø°)ô()¥µÁ°%‘•¹Ñ¥ÑåM••‘5…¹¥™•ÍÐì(€€€ÁÕˆ™¸¹•Ü¡Ù•ÉÍ¥½¸è¥µÁ°%¹Ñ¼ñMÑÉ¥¹œø°•¹ÑÉ¥•ÌèY•Œñ%‘•¹Ñ¥ÑåM••‘¹ÑÉäø¤€´øM•±˜ì(€€€€€€€M•±˜ì(€€€€€€€€€€€Ù•ÉÍ¥½¸èÙ•ÉÍ¥½¸¹¥¹Ñ¼ ¤°(€€€€€€€€€€€•¹ÑÉ¥•Ì°(€€€€€€€ô(€€€ô((€€€ÁÕˆ™¸Ù•ÉÍ¥½¸ ™Í•±˜¤€´ø€™ÍÑÈì(€€€€€€€€™Í•±˜¹Ù•ÉÍ¥½¸(€€€ô((€€€ÁÕˆ™¸•¹ÑÉ¥•Ì ™Í•±˜¤€´ø€™m%‘•¹Ñ¥ÑåM••‘¹ÑÉåtì(€€€€€€€€™Í•±˜¹•¹ÑÉ¥•Ì(€€€ô)ô((m‘•É¥Ù”¡•‰Õœ°±½¹”°A…ÉÑ¥…±Ä°Ä¥t)ÁÕˆÍÑÉÕÐÁÁ±å%‘•¹Ñ¥ÑåM••‘½µµ…¹ì(€€€½ÉÉ•±…Ñ¥½¹}¥è™…ÍÑ¥}‘½µ…¥¸èéI•ÅÕ•ÍÑ½ÉÉ•±…Ñ¥½¹%°(€€€…•ÍÌèI•ÅÕ•ÍÑ•ÍÍ½¹Ñ•áÐ°(€€€µ…¹¥™•ÍÐè%‘•¹Ñ¥ÑåM••‘5…¹¥™•ÍÐ°(€€€‘Éå}ÉÕ¸è‰½½°°)ô()¥µÁ°ÁÁ±å%‘•¹Ñ¥ÑåM••‘½µµ…¹ì(€€€ÁÕˆ™¸¹•Ü (€€€€€€€½ÉÉ•±…Ñ¥½¹}¥è™…ÍÑ¥}‘½µ…¥¸èéI•ÅÕ•ÍÑ½ÉÉ•±…Ñ¥½¹%°(€€€€€€€…•ÍÌèI•ÅÕ•ÍÑ•ÍÍ½¹Ñ•áÐ°(€€€€€€€µ…¹¥™•ÍÐè%‘•¹Ñ¥ÑåM••‘5…¹¥™•ÍÐ°(€€€€€€€‘Éå}ÉÕ¸è‰½½°°(€€€€¤€´øM•±˜ì(€€€€€€€M•±˜ì(€€€€€€€€€€€½ÉÉ•±…Ñ¥½¹}¥°(€€€€€€€€€€€…•ÍÌ°(€€€€€€€€€€€µ…¹¥™•ÍÐ°(€€€€€€€€€€€‘Éå}ÉÕ¸°(€€€€€€€ô(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸½ÉÉ•±…Ñ¥½¹}¥ ™Í•±˜¤€´ø™…ÍÑ¥}‘½µ…¥¸èéI•ÅÕ•ÍÑ½ÉÉ•±…Ñ¥½¹%ì(€€€€€€€Í•±˜¹½ÉÉ•±…Ñ¥½¹}¥(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸…•ÍÌ ™Í•±˜¤€´ø€™I•ÅÕ•ÍÑ•ÍÍ½¹Ñ•áÐì(€€€€€€€€™Í•±˜¹…•ÍÌ(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸µ…¹¥™•ÍÐ ™Í•±˜¤€´ø€™%‘•¹Ñ¥ÑåM••‘5…¹¥™•ÍÐì(€€€€€€€€™Í•±˜¹µ…¹¥™•ÍÐ(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸‘Éå}ÉÕ¸ ™Í•±˜¤€´ø‰½½°ì(€€€€€€€Í•±˜¹‘Éå}ÉÕ¸(€€€ô)ô((m‘•É¥Ù”¡•‰Õœ°½¹”°½Áä°A…ÉÑ¥…±Ä°Ä¥t)ÁÕˆ•¹Õ´%‘•¹Ñ¥ÑåM••‘¥ÍÁ½Í¥Ñ¥½¸ì(€€€]½Õ±‘É•…Ñ”°(€€€É•…Ñ•°(€€€I•ÕÍ•°(€€€½¹™±¥Ð°)ô((m‘•É¥Ù”¡•‰Õœ°±½¹”°A…ÉÑ¥…±Ä°Ä¥t)ÁÕˆÍÑÉÕÐ%‘•¹Ñ¥ÑåM••‘¹ÑÉå=ÕÑ½µ”ì(€€€­•äèMÑÉ¥¹œ°(€€€‘¥ÍÁ½Í¥Ñ¥½¸è%‘•¹Ñ¥ÑåM••‘¥ÍÁ½Í¥Ñ¥½¸°(€€€É•½É‘}¥è=ÁÑ¥½¸ñI•½É‘%ø°)ô()¥µÁ°%‘•¹Ñ¥ÑåM••‘¹ÑÉå=ÕÑ½µ”ì(€€€ÁÕˆ™¸¹•Ü (€€€€€€€­•äè¥µÁ°%¹Ñ¼ñMÑÉ¥¹œø°(€€€€€€€‘¥ÍÁ½Í¥Ñ¥½¸è%‘•¹Ñ¥ÑåM••‘¥ÍÁ½Í¥Ñ¥½¸°(€€€€€€€É•½É‘}¥è=ÁÑ¥½¸ñI•½É‘%ø°(€€€€¤€´øM•±˜ì(€€€€€€€M•±˜ì(€€€€€€€€€€€­•äè­•ä¹¥¹Ñ¼ ¤°(€€€€€€€€€€€‘¥ÍÁ½Í¥Ñ¥½¸°(€€€€€€€€€€€É•½É‘}¥°(€€€€€€€ô(€€€ô((€€€ÁÕˆ™¸­•ä ™Í•±˜¤€´ø€™ÍÑÈì(€€€€€€€€™Í•±˜¹­•ä(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸‘¥ÍÁ½Í¥Ñ¥½¸ ™Í•±˜¤€´ø%‘•¹Ñ¥ÑåM••‘¥ÍÁ½Í¥Ñ¥½¸ì(€€€€€€€Í•±˜¹‘¥ÍÁ½Í¥Ñ¥½¸(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸É•½É‘}¥ ™Í•±˜¤€´ø=ÁÑ¥½¸ñI•½É‘%øì(€€€€€€€Í•±˜¹É•½É‘}¥(€€€ô)ô((m‘•É¥Ù”¡•‰Õœ°±½¹”°A…ÉÑ¥…±Ä°Ä¥t)ÁÕˆÍÑÉÕÐÁÁ±å%‘•¹Ñ¥ÑåM••‘=ÕÑ½µ”ì(€€€Ù•ÉÍ¥½¸èMÑÉ¥¹œ°(€€€‘Éå}ÉÕ¸è‰½½°°(€€€•¹ÑÉ¥•ÌèY•Œñ%‘•¹Ñ¥ÑåM••‘¹ÑÉå=ÕÑ½µ”ø°)ô()¥µÁ°ÁÁ±å%‘•¹Ñ¥ÑåM••‘=ÕÑ½µ”ì(€€€ÁÕˆ™¸¹•Ü (€€€€€€€Ù•ÉÍ¥½¸è¥µÁ°%¹Ñ¼ñMÑÉ¥¹œø°(€€€€€€€‘Éå}ÉÕ¸è‰½½°°(€€€€€€€•¹ÑÉ¥•ÌèY•Œñ%‘•¹Ñ¥ÑåM••‘¹ÑÉå=ÕÑ½µ”ø°(€€€€¤€´øM•±˜ì(€€€€€€€M•±˜ì(€€€€€€€€€€€Ù•ÉÍ¥½¸èÙ•ÉÍ¥½¸¹¥¹Ñ¼ ¤°(€€€€€€€€€€€‘Éå}ÉÕ¸°(€€€€€€€€€€€•¹ÑÉ¥•Ì°(€€€€€€€ô(€€€ô((€€€ÁÕˆ™¸Ù•ÉÍ¥½¸ ™Í•±˜¤€´ø€™ÍÑÈì(€€€€€€€€™Í•±˜¹Ù•ÉÍ¥½¸(€€€ô((€€€ÁÕˆ½¹ÍÐ™¸‘Éå}ÉÕ¸ ™Í•±˜¤€´ø‰½½°ì(€€€€€€€Í•±˜¹‘Éå}ÉÕ¸(€€€ô((€€€ÁÕˆ™¸•¹ÑÉ¥•Ì ™Í•±˜¤€´ø€™m%‘•¹Ñ¥ÑåM••‘¹ÑÉå=ÕÑ½µ•tì(€€€€€€€€™Í•±˜¹•¹ÑÉ¥•Ì(€€€ô)ô()ÁÕˆÑÉ…¥Ð%‘•¹Ñ¥ÑåM••‘A½ÉÐèM•¹€¬Må¹Œì(€€€™¸…ÁÁ±å}¥‘•¹Ñ¥Ñå}Í•• (€€€€€€€€™Í•±˜°(€€€€€€€½µµ…¹èÁÁ±å%‘•¹Ñ¥ÑåM••‘½µµ…¹°(€€€€¤€´øÁÁ±¥…Ñ¥½¹I•ÍÕ±ÐñÁÁ±å%‘•¹Ñ¥ÑåM••‘=ÕÑ½µ”øì)ô()ÁÕˆÑÉ…¥Ð1½…±-•É¹•°è(€€€•ÍÍ‘µ¥¹¥ÍÑÉ…Ñ¥½¹A½ÉÐ(€€€€¬Ù¥‘•¹•UÁ±½…‘A½ÉÐ(€€€€¬%‘•¹Ñ¥ÑåA½ÉÐ(€€€€¬%‘•¹Ñ¥ÑåM••‘A½ÉÐ(€€€€¬É…Ñ”èé=‰Í•ÉÙ…Ñ¥½¹•ÁÑ…¹•A½ÉÐ(€€€€¬É…Ñ”èéI••¥ÁÑMÑÉ•…µA½ÉÐ(€€€€¬I•Ù¥•ÝA½ÉÐ(€€€€¬M•¹(€€€€¬Må¹Œ)ì)ô()¥µÁ°ñPø1½…±-•É¹•°™½ÈPÝ¡•É”(€€€Pè•ÍÍ‘µ¥¹¥ÍÑÉ…Ñ¥½¹A½ÉÐ(€€€€€€€€¬Ù¥‘•¹•UÁ±½…‘A½ÉÐ(€€€€€€€€¬%‘•¹Ñ¥ÑåA½ÉÐ(€€€€€€€€¬%‘•¹Ñ¥ÑåM••‘A½ÉÐ(€€€€€€€€¬É…Ñ”èé=‰Í•ÉÙ…Ñ¥½¹•ÁÑ…¹•A½ÉÐ(€€€€€€€€¬É…Ñ”èéI••¥ÁÑMÑÉ•…µA½ÉÐ(€€€€€€€€¬I•Ù¥•ÝA½ÉÐ(€€€€€€€€¬M•¹(€€€€€€€€¬Må¹Œ)ì)ô((m™œ¡Ñ•ÍÐ¥t)µ½Ñ•ÍÑÌì(€€€ÕÍ”ÍÕÁ•Èèè¨ì((€€€€mÑ•ÍÑt(€€€™¸Í•É•Ñ}É½Õ¹‘}ÑÉ¥Á}¥Í}•áÁ±¥¥Ñ}…¹‘}É•‘…Ñ•‘}‰å}ÑåÁ” ¤ì(€€€€€€€±•ÐÙ…±Õ”€ô€‰…ˆˆ¹É•Á•…Ð ÌÈ¤ì(€€€€€€€±•ÐÍ•É•Ð€ôM•É•Ñ5…Ñ•É¥…°èéÑÉå}™É½µ}¡•à ™Ù…±Õ”¤¹•áÁ•Ð ‰Ù…±¥Í•É•Ðˆ¤ì(€€€€€€€…ÍÍ•ÉÑ}•Ä„¡Í•É•Ð¹•áÁ½Í•}¡•à ¤°Ù…±Õ”¤ì(€€€€€€€…ÍÍ•ÉÐ„¡M•É•Ñ5…Ñ•É¥…°èéÑÉå}™É½µ}¡•à ˜‰ˆ¹É•Á•…Ð ÌÈ¤¤¹¥Í}•ÉÈ ¤¤ì(€€€ô((€€€€mÑ•ÍÑt(€€€™¸É•‘•¹Ñ¥…±}…ÕÑ¡•¹Ñ¥…Ñ¥½¹}­••ÁÍ}Ñ¡•}É•ÅÕ•ÍÑ•‘}ÁÉ½™¥±” ¤ì(€€€€€€€±•ÐÁÉ½™¥±•}¥€ôAÉ½™¥±•%èé¹•Ý}ØÜ ¤ì(€€€€€€€±•ÐÅÕ•Éä€ôÕÑ¡•¹Ñ¥…Ñ•É•‘•¹Ñ¥…±EÕ•Éäèé¹•Ü (€€€€€€€€€€€™…ÍÑ¥}‘½µ…¥¸èéI•ÅÕ•ÍÑ½ÉÉ•±…Ñ¥½¹%èé¹•Ý}ØÜ ¤°(€€€€€€€€€€€M•É•Ñ5…Ñ•É¥…°èéÑÉå}™É½µ}¡•à ˜‰…ˆˆ¹É•Á•…Ð ÌÈ¤¤¹•áÁ•Ð ‰Ù…±¥Í•É•Ðˆ¤°(€€€€€€€€€€€ÁÉ½™¥±•}¥°(€€€€€€€€¤ì((€€€€€€€…ÍÍ•ÉÑ}•Ä„¡ÅÕ•Éä¹ÁÉ½™¥±•}¥ ¤°ÁÉ½™¥±•}¥¤ì(€€€ô)ô(