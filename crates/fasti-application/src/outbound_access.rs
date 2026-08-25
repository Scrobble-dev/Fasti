//! Narrowing-only policy for provider and other governed outbound access.
//!
//! A declaration is the maximum grant. Operator allow lists can only narrow
//! it, and a deny always wins. Transport adapters must still disable redirects,
//! resolve and authorize every DNS answer, then connect to an authorized address
//! without resolving the host again or consulting a system proxy.

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

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
    Reserved,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
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
pub struct OutboundAccessDeclaration<'a> {
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
    declaration: OutboundAccessDeclaration<'_>,
    policy: &OutboundAccessPolicy,
    capability: &str,
    host: &str,
    addresses: &[IpAddr],
) -> Result<(), AccessDenial> {
    require_value("provider", declaration.provider)?;
    require_value("capability", capability)?;
    canonical_host(host).map_err(|()| denial("host", host))?;
    require_declared("capability", capability, declaration.capabilities, false)?;
    require_declared("host", host, declaration.hosts, true)?;
    narrow(
        "provider",
        declaration.provider,
        &policy.allow_providers,
        &policy.deny_providers,
        false,
    )?;
    narrow(
        "capability",
        capability,
        &policy.allow_capabilities,
        &policy.deny_capabilities,
        false,
    )?;
    narrow("host", host, &policy.allow_hosts, &policy.deny_hosts, true)?;
    if addresses.is_empty() {
        return Err(denial("network", "no resolved addresses"));
    }
    for address in addresses {
        let class = network_class(*address);
        if !declaration.networks.contains(&class)
            || policy.deny_networks.contains(&class)
            || (!policy.allow_networks.is_empty() && !policy.allow_networks.contains(&class))
        {
            return Err(denial("network", format!("{address} ({class:?})")));
        }
    }
    Ok(())
}

fn require_value(dimension: &'static str, value: &str) -> Result<(), AccessDenial> {
    if value.trim().is_empty() {
        Err(denial(dimension, "empty value"))
    } else {
        Ok(())
    }
}

fn require_declared(
    dimension: &'static str,
    value: &str,
    declared: &[&str],
    host: bool,
) -> Result<(), AccessDenial> {
    if declared.iter().any(|item| matches(item, value, host)) {
        Ok(())
    } else {
        Err(denial(dimension, value))
    }
}

fn narrow(
    dimension: &'static str,
    value: &str,
    allow: &[String],
    deny: &[String],
    host: bool,
) -> Result<(), AccessDenial> {
    if deny.iter().any(|item| matches(item, value, host))
        || (!allow.is_empty() && !allow.iter().any(|item| matches(item, value, host)))
    {
        Err(denial(dimension, value))
    } else {
        Ok(())
    }
}

fn matches(left: &str, right: &str, host: bool) -> bool {
    if host {
        match (canonical_host(left), canonical_host(right)) {
            (Ok(left), Ok(right)) => left.eq_ignore_ascii_case(right),
            _ => false,
        }
    } else {
        left == right
    }
}

fn canonical_host(host: &str) -> Result<&str, ()> {
    let host = host.strip_suffix('.').unwrap_or(host);
    if host.is_empty() || host.ends_with('.') {
        Err(())
    } else {
        Ok(host)
    }
}

fn denial(dimension: &'static str, value: impl Into<String>) -> AccessDenial {
    AccessDenial {
        dimension,
        value: value.into(),
    }
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
    let octets = address.octets();
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
    } else if octets[0] == 0
        || (octets[0] == 100 && octets[1] & 0xc0 == 0x40)
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 192 && octets[1] == 88 && octets[2] == 99)
        || (octets[0] == 198 && octets[1] & 0xfe == 18)
        || octets[0] >= 240
    {
        NetworkClass::Reserved
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
    } else if segments[0] == 0x2002
        || (segments[0] == 0x2001 && segments[1] <= 0x01ff)
        || segments[0] & 0xe000 != 0x2000
    {
        NetworkClass::Reserved
    } else {
        NetworkClass::Public
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCESS: OutboundAccessDeclaration<'static> = OutboundAccessDeclaration {
        provider: "google-books",
        capabilities: &["metadata.search"],
        hosts: &["www.googleapis.com"],
        networks: &[NetworkClass::Public],
    };

    fn public_addresses() -> [IpAddr; 1] {
        ["142.250.74.74".parse().expect("public address")]
    }

    #[test]
    fn default_policy_accepts_only_declared_public_access() {
        assert!(authorize_outbound(
            ACCESS,
            &OutboundAccessPolicy::default(),
            "metadata.search",
            "WWW.GOOGLEAPIS.COM.",
            &public_addresses(),
        )
        .is_ok());
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "224.0.0.1",
            "0.0.0.0",
            "192.0.2.1",
            "100.64.0.1",
            "198.18.0.1",
            "192.88.99.1",
            "240.0.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "2002:7f00:0001::",
            "2001:100::1",
            "::ffff:10.0.0.1",
        ] {
            assert!(
                authorize_outbound(
                    ACCESS,
                    &OutboundAccessPolicy::default(),
                    "metadata.search",
                    "www.googleapis.com",
                    &[address.parse().expect("classified address")],
                )
                .is_err(),
                "accepted {address}",
            );
        }
    }

    #[test]
    fn operator_allow_lists_cannot_widen_a_declaration() {
        let policy = OutboundAccessPolicy {
            allow_hosts: vec!["metadata.internal".to_owned()],
            allow_networks: vec![NetworkClass::Private],
            ..OutboundAccessPolicy::default()
        };
        assert_eq!(
            authorize_outbound(
                ACCESS,
                &policy,
                "metadata.search",
                "metadata.internal",
                &["10.0.0.2".parse().expect("private address")],
            )
            .expect_err("undeclared host")
            .dimension(),
            "host",
        );
    }

    #[test]
    fn deny_wins_for_every_configurable_dimension() {
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
                deny_hosts: vec!["WWW.GOOGLEAPIS.COM.".to_owned()],
                ..OutboundAccessPolicy::default()
            },
            OutboundAccessPolicy {
                deny_networks: vec![NetworkClass::Public],
                ..OutboundAccessPolicy::default()
            },
        ] {
            assert!(authorize_outbound(
                ACCESS,
                &policy,
                "metadata.search",
                "www.googleapis.com",
                &public_addresses(),
            )
            .is_err());
        }
    }

    #[test]
    fn missing_or_partial_resolution_fails_closed() {
        assert_eq!(
            authorize_outbound(
                ACCESS,
                &OutboundAccessPolicy::default(),
                "metadata.search",
                "www.googleapis.com",
                &[],
            )
            .expect_err("empty DNS result")
            .dimension(),
            "network",
        );
        assert!(authorize_outbound(
            ACCESS,
            &OutboundAccessPolicy::default(),
            "metadata.search",
            "www.googleapis.com",
            &[
                "142.250.74.74".parse().expect("public address"),
                "127.0.0.1".parse().expect("loopback address"),
            ],
        )
        .is_err());
    }

    #[test]
    fn empty_request_dimensions_fail_closed() {
        let empty_provider = OutboundAccessDeclaration {
            provider: "",
            ..ACCESS
        };
        for result in [
            authorize_outbound(
                empty_provider,
                &OutboundAccessPolicy::default(),
                "metadata.search",
                "www.googleapis.com",
                &public_addresses(),
            ),
            authorize_outbound(
                ACCESS,
                &OutboundAccessPolicy::default(),
                "",
                "www.googleapis.com",
                &public_addresses(),
            ),
            authorize_outbound(
                ACCESS,
                &OutboundAccessPolicy::default(),
                "metadata.search",
                ".",
                &public_addresses(),
            ),
        ] {
            assert!(result.is_err());
        }
    }

    #[test]
    fn identifiers_are_exact_and_hosts_accept_one_terminal_dot() {
        for (capability, host) in [
            ("METADATA.SEARCH", "www.googleapis.com"),
            ("metadata.search", "www.googleapis.com.."),
        ] {
            assert!(authorize_outbound(
                ACCESS,
                &OutboundAccessPolicy::default(),
                capability,
                host,
                &public_addresses(),
            )
            .is_err());
        }
        assert!(authorize_outbound(
            ACCESS,
            &OutboundAccessPolicy {
                deny_providers: vec!["GOOGLE-BOOKS".to_owned()],
                deny_capabilities: vec!["METADATA.SEARCH".to_owned()],
                ..OutboundAccessPolicy::default()
            },
            "metadata.search",
            "www.googleapis.com",
            &public_addresses(),
        )
        .is_ok());
    }

    #[test]
    fn unknown_policy_fields_fail_deserialization() {
        assert!(serde_json::from_str::<OutboundAccessPolicy>(
            r#"{"deny_host":["www.googleapis.com"]}"#,
        )
        .is_err());
    }
}
