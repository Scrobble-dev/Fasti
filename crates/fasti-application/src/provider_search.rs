//! Neutral provider-search results and narrowing-only outbound access policy.
//!
//! Provider declarations define the maximum grant. Operator settings can only
//! narrow that declaration, and a deny always wins. HTTP adapters still own
//! DNS pinning, redirect handling, timeouts, and response-size limits.

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

const MAX_CANDIDATE_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderCandidate {
    provider: String,
    provider_id: String,
    title: String,
    kind: String,
    description: Option<String>,
    image_url: Option<String>,
}

impl ProviderCandidate {
    pub fn try_new(
        provider: impl Into<String>,
        provider_id: impl Into<String>,
        title: impl Into<String>,
        kind: impl Into<String>,
        description: Option<String>,
        image_url: Option<String>,
    ) -> Result<Self, ProviderCandidateError> {
        let provider = provider.into();
        let provider_id = provider_id.into();
        let title = title.into();
        let kind = kind.into();
        for value in [&provider, &provider_id, &title, &kind] {
            validate_candidate_text(value)?;
        }
        if let Some(value) = description.as_deref() {
            validate_candidate_text(value)?;
        }
        if let Some(value) = image_url.as_deref() {
            validate_candidate_text(value)?;
        }
        Ok(Self {
            provider,
            provider_id,
            title,
            kind,
            description,
            image_url,
        })
    }
}

fn validate_candidate_text(value: &str) -> Result<(), ProviderCandidateError> {
    if value.is_empty()
        || value.len() > MAX_CANDIDATE_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ProviderCandidateError);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCandidateError;

impl std::fmt::Display for ProviderCandidateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(
            "provider candidate fields must be non-empty, bounded, and contain no control characters",
        )
    }
}

impl std::error::Error for ProviderCandidateError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkClass {
    Public,
    Loopback,
    Private,
    LinkLocal,
    Multicast,
    Unspecified,
    Documentation,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct OutboundAccessPolicy {
    #[serde(default)]
    pub allow_providers: Vec<String>,
    #[serde(default)]
    pub deny_providers: Vec<String>,
    #[serde(default)]
    pub allow_capabilities: Vec<String>,
    #[serde(default)]
    pub deny_capabilities: Vec<String>,
    #[serde(default)]
    pub allow_hosts: Vec<String>,
    #[serde(default)]
    pub deny_hosts: Vec<String>,
    #[serde(default)]
    pub allow_networks: Vec<NetworkClass>,
    #[serde(default)]
    pub deny_networks: Vec<NetworkClass>,
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderAccessDeclaration<'a> {
    pub provider: &'a str,
    pub capabilities: &'a [&'a str],
    pub hosts: &'a [&'a str],
    pub networks: &'a [NetworkClass],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AccessDenial {
    dimension: &'static str,
    value: String,
}

impl AccessDenial {
    pub fn dimension(&self) -> &'static str {
        self.dimension
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

pub fn authorize_outbound(
    declaration: ProviderAccessDeclaration<'_>,
    policy: &OutboundAccessPolicy,
    capability: &str,
    host: &str,
    addresses: &[IpAddr],
) -> Result<(), AccessDenial> {
    require_declared("capability", capability, declaration.capabilities)?;
    require_declared("host", host, declaration.hosts)?;
    narrow(
        "provider",
        declaration.provider,
        &policy.allow_providers,
        &policy.deny_providers,
    )?;
    narrow(
        "capability",
        capability,
        &policy.allow_capabilities,
        &policy.deny_capabilities,
    )?;
    narrow("host", host, &policy.allow_hosts, &policy.deny_hosts)?;
    for address in addresses {
        let class = network_class(*address);
        if !declaration.networks.contains(&class)
            || policy.deny_networks.contains(&class)
            || (!policy.allow_networks.is_empty() && !policy.allow_networks.contains(&class))
        {
            return Err(AccessDenial {
                dimension: "network",
                value: format!("{address} ({class:?})"),
            });
        }
    }
    Ok(())
}

fn require_declared(
    dimension: &'static str,
    value: &str,
    declared: &[&str],
) -> Result<(), AccessDenial> {
    if declared.contains(&value) {
        Ok(())
    } else {
        Err(AccessDenial {
            dimension,
            value: value.to_owned(),
        })
    }
}

fn narrow(
    dimension: &'static str,
    value: &str,
    allow: &[String],
    deny: &[String],
) -> Result<(), AccessDenial> {
    if deny.iter().any(|item| item.eq_ignore_ascii_case(value))
        || (!allow.is_empty() && !allow.iter().any(|item| item.eq_ignore_ascii_case(value)))
    {
        return Err(AccessDenial {
            dimension,
            value: value.to_owned(),
        });
    }
    Ok(())
}

pub fn network_class(address: IpAddr) -> NetworkClass {
    match address {
        IpAddr::V4(address) => ipv4_class(address),
        IpAddr::V6(address) => address
            .to_ipv4_mapped()
            .map(ipv4_class)
            .unwrap_or_else(|| ipv6_class(address)),
    }
}

fn ipv4_class(address: Ipv4Addr) -> NetworkClass {
    if address.is_loopback() {
        NetworkClass::Loopback
    } else if address.is_private() {
        NetworkClass::Private
    } else if address.is_link_local() {
        NetworkClass::LinkLocal
    } else if address.is_multicast() || address == Ipv4Addr::BROADCAST {
        NetworkClass::Multicast
    } else if address.is_unspecified() {
        NetworkClass::Unspecified
    } else if address.is_documentation() {
        NetworkClass::Documentation
    } else {
        NetworkClass::Public
    }
}

fn ipv6_class(address: Ipv6Addr) -> NetworkClass {
    let segments = address.segments();
    if address.is_loopback() {
        NetworkClass::Loopback
    } else if segments[0] & 0xfe00 == 0xfc00 {
        NetworkClass::Private
    } else if segments[0] & 0xffc0 == 0xfe80 {
        NetworkClass::LinkLocal
    } else if address.is_multicast() {
        NetworkClass::Multicast
    } else if address.is_unspecified() {
        NetworkClass::Unspecified
    } else if segments[0] == 0x2001 && segments[1] == 0x0db8 {
        NetworkClass::Documentation
    } else {
        NetworkClass::Public
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECLARATION: ProviderAccessDeclaration<'static> = ProviderAccessDeclaration {
        provider: "google-books",
        capabilities: &["metadata.search"],
        hosts: &["www.googleapis.com"],
        networks: &[NetworkClass::Public],
    };

    #[test]
    fn user_allows_cannot_widen_a_provider_declaration() {
        let policy = OutboundAccessPolicy {
            allow_hosts: vec!["metadata.internal".to_owned()],
            allow_networks: vec![NetworkClass::Private],
            ..OutboundAccessPolicy::default()
        };
        assert_eq!(
            authorize_outbound(
                DECLARATION,
                &policy,
                "metadata.search",
                "metadata.internal",
                &["10.0.0.2".parse().expect("private address")],
            )
            .expect_err("undeclared host")
            .dimension(),
            "host"
        );
    }

    #[test]
    fn deny_wins_for_every_configurable_dimension() {
        let public = ["142.250.74.74".parse().expect("public address")];
        for policy in [
            OutboundAccessPolicy {
                deny_providers: vec!["google-books".to_owned()],
                ..OutboundAccessPolicy::default()
            },
            OutboundAccessPolicy {
                deny_capabilities: vec!["metadata.search".to_owned()],
                ..OutboundAccessPolicy::default()
            },
            OutboundAccessPolicy {
                deny_hosts: vec!["www.googleapis.com".to_owned()],
                ..OutboundAccessPolicy::default()
            },
            OutboundAccessPolicy {
                deny_networks: vec![NetworkClass::Public],
                ..OutboundAccessPolicy::default()
            },
        ] {
            assert!(authorize_outbound(
                DECLARATION,
                &policy,
                "metadata.search",
                "www.googleapis.com",
                &public,
            )
            .is_err());
        }
    }

    #[test]
    fn default_policy_accepts_only_the_declared_public_destination() {
        assert!(authorize_outbound(
            DECLARATION,
            &OutboundAccessPolicy::default(),
            "metadata.search",
            "www.googleapis.com",
            &["142.250.74.74".parse().expect("public address")],
        )
        .is_ok());
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "224.0.0.1",
            "0.0.0.0",
            "192.0.2.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
        ] {
            assert!(
                authorize_outbound(
                    DECLARATION,
                    &OutboundAccessPolicy::default(),
                    "metadata.search",
                    "www.googleapis.com",
                    &[address.parse().expect("classified address")],
                )
                .is_err(),
                "accepted {address}"
            );
        }
    }

    #[test]
    fn candidates_are_bounded_and_neutral() {
        assert!(ProviderCandidate::try_new(
            "google-books",
            "volume-1",
            "A Book",
            "book",
            None,
            None,
        )
        .is_ok());
        assert!(ProviderCandidate::try_new(
            "google-books",
            "volume-1",
            "x".repeat(MAX_CANDIDATE_TEXT_BYTES + 1),
            "book",
            None,
            None,
        )
        .is_err());
    }
}
