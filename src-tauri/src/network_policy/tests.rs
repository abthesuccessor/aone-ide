use std::net::{IpAddr, Ipv6Addr};

use super::address::IANA_IPV6_SPECIAL_RANGES;
use super::{DestinationScope, classify_destination, is_cloud_metadata_hostname};

const IANA_REGISTRY_PREFIXES: &[(&str, u8)] = &[
    ("::1", 128),
    ("::", 128),
    ("2001:1::1", 128),
    ("2001:1::2", 128),
    ("2001:1::3", 128),
    ("::ffff:0:0", 96),
    ("64:ff9b::", 96),
    ("100::", 64),
    ("100:0:0:1::", 64),
    ("64:ff9b:1::", 48),
    ("2001:2::", 48),
    ("2001:4:112::", 48),
    ("2620:4f:8000::", 48),
    ("2001::", 32),
    ("2001:3::", 32),
    ("2001:db8::", 32),
    ("2001:10::", 28),
    ("2001:20::", 28),
    ("2001:30::", 28),
    ("2001::", 23),
    ("3fff::", 20),
    ("2002::", 16),
    ("5f00::", 16),
    ("fe80::", 10),
    ("fc00::", 7),
];

#[test]
fn registry_table_matches_the_2025_10_09_iana_snapshot() {
    let actual = IANA_IPV6_SPECIAL_RANGES
        .iter()
        .map(|range| (range.network, range.prefix_length))
        .collect::<Vec<_>>();
    let expected = IANA_REGISTRY_PREFIXES
        .iter()
        .map(|(network, prefix)| {
            (
                network.parse::<Ipv6Addr>().expect("registry prefix"),
                *prefix,
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn classifies_public_private_and_loopback_addresses() {
    for address in [
        "8.8.8.8",
        "1.1.1.1",
        "2001:200::1",
        "2001:4860:4860::8888",
        "2606:4700:4700::1111",
    ] {
        assert_eq!(scope(address), DestinationScope::Public, "{address}");
    }
    for address in ["10.0.0.1", "172.16.0.1", "192.168.1.1", "fd12::1"] {
        assert_eq!(scope(address), DestinationScope::Private, "{address}");
    }
    for address in ["127.0.0.1", "::1", "::ffff:127.0.0.1"] {
        assert_eq!(scope(address), DestinationScope::Loopback, "{address}");
    }
}

#[test]
fn prohibits_every_non_warning_iana_ipv6_special_range() {
    for address in [
        "::",
        "64:ff9b::c000:201",
        "64:ff9b:1::1",
        "100::1",
        "100:0:0:1::1",
        "2001::1",
        "2001:1::1",
        "2001:1::2",
        "2001:1::3",
        "2001:2::1",
        "2001:3::1",
        "2001:4:112::1",
        "2001:10::1",
        "2001:20::1",
        "2001:30::1",
        "2001:db8::1",
        "2002::1",
        "2620:4f:8000::1",
        "3fff:fff::1",
        "5f00::1",
        "fe80::1",
    ] {
        assert_prohibited(address);
    }
}

#[test]
fn ipv4_embedded_addresses_follow_the_embedded_ipv4_scope() {
    for address in ["::ffff:8.8.8.8", "::8.8.8.8"] {
        assert_eq!(scope(address), DestinationScope::Public, "{address}");
    }
    for address in ["::ffff:10.0.0.1", "::10.0.0.1"] {
        assert_eq!(scope(address), DestinationScope::Private, "{address}");
    }
    for address in ["::ffff:127.0.0.1", "::127.0.0.1"] {
        assert_eq!(scope(address), DestinationScope::Loopback, "{address}");
    }
    for address in ["::ffff:169.254.169.254", "::169.254.169.254"] {
        assert_prohibited(address);
    }
}

#[test]
fn prohibits_other_non_public_ipv6_scopes() {
    for address in ["fd00:ec2::254", "fd20:ce::254", "fec0::1", "ff02::1"] {
        assert_prohibited(address);
    }
}

#[test]
fn prohibits_metadata_link_local_multicast_and_unspecified_ipv4_addresses() {
    for address in ["0.0.0.0", "169.254.169.254", "224.0.0.1", "255.255.255.255"] {
        assert_prohibited(address);
    }
}

#[test]
fn prohibits_non_public_special_purpose_ipv4_ranges() {
    for address in [
        "100.64.0.1",
        "192.0.0.1",
        "192.0.2.1",
        "192.88.99.1",
        "198.18.0.1",
        "198.51.100.1",
        "203.0.113.1",
        "240.0.0.1",
    ] {
        assert_prohibited(address);
    }
}

#[test]
fn recognizes_only_explicit_cloud_metadata_hostnames() {
    for hostname in [
        "metadata.google.internal",
        "metadata.google.internal.",
        "METADATA.GOOGLE.INTERNAL",
        "metadata.goog",
        "instance-data.ec2.internal",
    ] {
        assert!(is_cloud_metadata_hostname(hostname), "{hostname}");
    }
    for hostname in [
        "example.com",
        "metadata.google.internal.example.com",
        "not-metadata.goog",
    ] {
        assert!(!is_cloud_metadata_hostname(hostname), "{hostname}");
    }
}

fn scope(address: &str) -> DestinationScope {
    classify_destination(address.parse::<IpAddr>().expect("IP literal"))
}

fn assert_prohibited(address: &str) {
    assert!(
        matches!(scope(address), DestinationScope::Prohibited(_)),
        "{address} must be prohibited"
    );
}
