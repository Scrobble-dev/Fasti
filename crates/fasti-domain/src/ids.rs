use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};
use thiserror::Error;
use uuid::Uuid;

/// Lifecycle of an identifier type in the current executable body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdLifecycle {
    Executable,
    Reserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdPrefixSpec {
    pub kind: IdKind,
    pub type_name: &'static str,
    pub prefix: &'static str,
    pub lifecycle: IdLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdError {
    #[error("identifier prefix is not registered")]
    UnknownPrefix,
    #[error("expected {expected} identifier prefix, found {actual}")]
    WrongType {
        expected: &'static str,
        actual: &'static str,
    },
    #[error("identifier UUID is malformed")]
    MalformedUuid,
    #[error("identifier UUID must be version 7")]
    NotVersion7,
    #[error("identifier must use the canonical lowercase compact representation")]
    NonCanonical,
}

/// Shared strict codec used by every concrete identifier newtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PrefixedUuidV7 {
    kind: IdKind,
    uuid: Uuid,
}

impl PrefixedUuidV7 {
    pub fn new(kind: IdKind) -> Self {
        Self {
            kind,
            uuid: Uuid::now_v7(),
        }
    }

    pub fn from_uuid(kind: IdKind, uuid: Uuid) -> Result<Self, IdError> {
        if uuid.get_version_num() != 7 {
            return Err(IdError::NotVersion7);
        }
        Ok(Self { kind, uuid })
    }

    pub fn kind(self) -> IdKind {
        self.kind
    }

    pub fn uuid(self) -> Uuid {
        self.uuid
    }

    fn spec(self) -> &'static IdPrefixSpec {
        spec_for_kind(self.kind)
    }
}

impl fmt::Display for PrefixedUuidV7 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.spec().prefix, self.uuid.simple())
    }
}

impl FromStr for PrefixedUuidV7 {
    type Err = IdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let spec = ID_PREFIX_REGISTRY
            .iter()
            .find(|entry| value.starts_with(entry.prefix))
            .ok_or(IdError::UnknownPrefix)?;
        let raw_uuid = &value[spec.prefix.len()..];
        let uuid = Uuid::parse_str(raw_uuid).map_err(|_| IdError::MalformedUuid)?;
        let parsed = Self::from_uuid(spec.kind, uuid)?;
        if parsed.to_string() != value {
            return Err(IdError::NonCanonical);
        }
        Ok(parsed)
    }
}

impl Serialize for PrefixedUuidV7 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for PrefixedUuidV7 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

pub fn spec_for_kind(kind: IdKind) -> &'static IdPrefixSpec {
    ID_PREFIX_REGISTRY
        .iter()
        .find(|entry| entry.kind == kind)
        .expect("every IdKind must have one registry entry")
}

macro_rules! define_fasti_ids {
    ($(($kind:ident, $name:ident, $prefix:literal, $lifecycle:ident)),+ $(,)?) => {
        /// Stable semantic owner of a typed Fasti identifier prefix.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum IdKind {
            $($kind),+
        }

        /// Version-one prefix registry. A reserved type is parseable but does
        /// not gain a command, route, or persistence path merely by appearing.
        pub const ID_PREFIX_REGISTRY: &[IdPrefixSpec] = &[
            $(IdPrefixSpec {
                kind: IdKind::$kind,
                type_name: stringify!($name),
                prefix: $prefix,
                lifecycle: IdLifecycle::$lifecycle,
            }),+
        ];

        $(
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(PrefixedUuidV7);

        impl $name {
            pub fn new_v7() -> Self {
                Self(PrefixedUuidV7::new(IdKind::$kind))
            }

            pub fn from_uuid(uuid: Uuid) -> Result<Self, IdError> {
                PrefixedUuidV7::from_uuid(IdKind::$kind, uuid).map(Self)
            }

            pub fn uuid(self) -> Uuid {
                self.0.uuid()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new_v7()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = IdError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                let parsed: PrefixedUuidV7 = value.parse()?;
                if parsed.kind() != IdKind::$kind {
                    return Err(IdError::WrongType {
                        expected: spec_for_kind(IdKind::$kind).prefix,
                        actual: parsed.spec().prefix,
                    });
                }
                Ok(Self(parsed))
            }
        }
        )+
    };
}

define_fasti_ids!(
    (Workspace, WorkspaceId, "wsp_", Executable),
    (Profile, ProfileId, "prf_", Executable),
    (Client, ClientId, "cli_", Executable),
    (Credential, CredentialId, "crd_", Executable),
    (ProfileGrant, ProfileGrantId, "grt_", Executable),
    (Record, RecordId, "rec_", Reserved),
    (ExternalIdentifier, ExternalIdentifierId, "xid_", Reserved),
    (IdentityAssertion, IdentityAssertionId, "asr_", Reserved),
    (Evidence, EvidenceId, "evd_", Executable),
    (Observation, ObservationId, "obs_", Executable),
    (Occurrence, OccurrenceId, "occ_", Reserved),
    (Interpretation, InterpretationId, "int_", Reserved),
    (ReviewItem, ReviewItemId, "rev_", Reserved),
    (Operation, OperationId, "op_", Executable),
    (Receipt, ReceiptId, "rcp_", Executable),
    (Correction, CorrectionId, "cor_", Reserved),
    (RestoreAttempt, RestoreAttemptId, "rst_", Reserved),
    (RequestCorrelation, RequestCorrelationId, "req_", Executable),
    (FieldDefinition, FieldDefinitionId, "fld_", Reserved),
);

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashSet;

    #[test]
    fn registry_has_one_unique_prefix_per_kind() {
        assert_eq!(ID_PREFIX_REGISTRY.len(), 19);
        let prefixes: HashSet<_> = ID_PREFIX_REGISTRY
            .iter()
            .map(|entry| entry.prefix)
            .collect();
        let kinds: HashSet<_> = ID_PREFIX_REGISTRY.iter().map(|entry| entry.kind).collect();
        let names: HashSet<_> = ID_PREFIX_REGISTRY
            .iter()
            .map(|entry| entry.type_name)
            .collect();
        assert_eq!(prefixes.len(), ID_PREFIX_REGISTRY.len());
        assert_eq!(kinds.len(), ID_PREFIX_REGISTRY.len());
        assert_eq!(names.len(), ID_PREFIX_REGISTRY.len());
    }

    #[test]
    fn executable_and_reserved_assignments_are_exact() {
        let executable = ID_PREFIX_REGISTRY
            .iter()
            .filter(|entry| entry.lifecycle == IdLifecycle::Executable)
            .map(|entry| entry.prefix)
            .collect::<HashSet<_>>();
        let reserved = ID_PREFIX_REGISTRY
            .iter()
            .filter(|entry| entry.lifecycle == IdLifecycle::Reserved)
            .map(|entry| entry.prefix)
            .collect::<HashSet<_>>();

        assert_eq!(
            executable,
            HashSet::from([
                "wsp_", "prf_", "cli_", "crd_", "grt_", "evd_", "obs_", "op_", "rcp_", "req_",
            ])
        );
        assert_eq!(
            reserved,
            HashSet::from([
                "rec_", "xid_", "asr_", "occ_", "int_", "rev_", "cor_", "rst_", "fld_",
            ])
        );
    }

    #[test]
    fn every_registered_kind_uses_the_shared_round_trip_codec() {
        for spec in ID_PREFIX_REGISTRY {
            let id = PrefixedUuidV7::new(spec.kind);
            let encoded = id.to_string();
            assert!(encoded.starts_with(spec.prefix));
            assert_eq!(encoded.parse::<PrefixedUuidV7>().expect("round trip"), id);
        }
    }

    proptest! {
        #[test]
        fn every_v7_uuid_round_trips_through_a_typed_id(mut bytes in any::<[u8; 16]>()) {
            bytes[6] = (bytes[6] & 0x0f) | 0x70;
            bytes[8] = (bytes[8] & 0x3f) | 0x80;
            let uuid = Uuid::from_bytes(bytes);
            let typed = ObservationId::from_uuid(uuid).expect("version bits are v7");
            let encoded = typed.to_string();

            prop_assert_eq!(encoded.parse::<ObservationId>(), Ok(typed));
            prop_assert!(encoded.parse::<RecordId>().is_err());
        }
    }

    #[test]
    fn typed_id_round_trips_canonically() {
        let id = RecordId::new_v7();
        let encoded = id.to_string();
        assert!(encoded.starts_with("rec_"));
        assert_eq!(encoded.len(), 36);
        assert_eq!(encoded.parse::<RecordId>().expect("valid record id"), id);
    }

    #[test]
    fn cross_type_use_is_rejected() {
        let record = RecordId::new_v7().to_string();
        assert!(matches!(
            record.parse::<ObservationId>(),
            Err(IdError::WrongType { .. })
        ));
    }

    #[test]
    fn non_v7_and_noncanonical_ids_are_rejected() {
        let v4 = Uuid::new_v4();
        let value = format!("rec_{}", v4.simple());
        assert_eq!(value.parse::<RecordId>(), Err(IdError::NotVersion7));

        let uppercase = RecordId::new_v7().to_string().to_uppercase();
        assert!(uppercase.parse::<RecordId>().is_err());
    }

    #[test]
    fn unknown_prefix_and_malformed_uuid_are_rejected() {
        assert_eq!(
            "wat_018f0e0e7f7b70008000000000000000".parse::<PrefixedUuidV7>(),
            Err(IdError::UnknownPrefix)
        );
        assert_eq!(
            "rec_not-a-uuid".parse::<RecordId>(),
            Err(IdError::MalformedUuid)
        );
    }
}
