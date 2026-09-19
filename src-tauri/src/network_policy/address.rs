use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DestinationScope {
    Public,
    Private,
    Loopback,
    Prohibited(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Ipv6SpecialRange {
    pub(super) network: Ipv6Addr,
    pub(super) prefix_length: u8,
    scope: DestinationScope,
}

impl Ipv6SpecialRange {
    fn contains(self, ip: Ipv6Addr) -> bool {
        let shift = 128 - u32::from(self.prefix_length);
        let mask = u128::MAX << shift;
        (u128::from(ip) & mask) == (u128::from(self.network) & mask)
    }
}

const fn special(
    network: Ipv6Addr,
    prefix_length: u8,
    scope: DestinationScope,
) -> Ipv6SpecialRange {
    Ipv6SpecialRange {
        network,
        prefix_length,
        scope,
    }
}

// IANA IPv6 Special-Purpose Address Space, last updated 2025-10-09:
// https://www.iana.org/assignments/iana-ipv6-special-registry/
// Specific entries precede their containing allocations for useful diagnostics.
pub(super) const IANA_IPV6_SPECIAL_RANGES: &[Ipv6SpecialRange] = &[
    special(Ipv6Addr::LOCALHOST, 128, DestinationScope::Loopback),
    special(
        Ipv6Addr::UNSPECIFIED,
        128,
        DestinationScope::Prohibited("unspecified address"),
    ),
    special(
        Ipv6Addr::new(0x2001, 1, 0, 0, 0, 0, 0, 1),
        128,
        DestinationScope::Prohibited("PCP anycast address"),
    ),
    special(
        Ipv6Addr::new(0x2001, 1, 0, 0, 0, 0, 0, 2),
        128,
        DestinationScope::Prohibited("TURN anycast address"),
    ),
    special(
        Ipv6Addr::new(0x2001, 1, 0, 0, 0, 0, 0, 3),
        128,
        DestinationScope::Prohibited("DNS-SD anycast address"),
    ),
    special(
        Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0, 0),
        96,
        DestinationScope::Prohibited("IPv4-mapped address"),
    ),
    special(
        Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, 0, 0),
        96,
        DestinationScope::Prohibited("IPv4-IPv6 translation address"),
    ),
    special(
        Ipv6Addr::new(0x100, 0, 0, 0, 0, 0, 0, 0),
        64,
        DestinationScope::Prohibited("discard-only address"),
    ),
    special(
        Ipv6Addr::new(0x100, 0, 0, 1, 0, 0, 0, 0),
        64,
        DestinationScope::Prohibited("dummy IPv6 prefix"),
    ),
    special(
        Ipv6Addr::new(0x64, 0xff9b, 1, 0, 0, 0, 0, 0),
        48,
        DestinationScope::Prohibited("local IPv4-IPv6 translation address"),
    ),
    special(
        Ipv6Addr::new(0x2001, 2, 0, 0, 0, 0, 0, 0),
        48,
        DestinationScope::Prohibited("benchmarking address"),
    ),
    special(
        Ipv6Addr::new(0x2001, 4, 0x112, 0, 0, 0, 0, 0),
        48,
        DestinationScope::Prohibited("AS112-v6 address"),
    ),
    special(
        Ipv6Addr::new(0x2620, 0x4f, 0x8000, 0, 0, 0, 0, 0),
        48,
        DestinationScope::Prohibited("direct delegation AS112 address"),
    ),
    special(
        Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 0),
        32,
        DestinationScope::Prohibited("Teredo address"),
    ),
    special(
        Ipv6Addr::new(0x2001, 3, 0, 0, 0, 0, 0, 0),
        32,
        DestinationScope::Prohibited("AMT address"),
    ),
    special(
        Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0),
        32,
        DestinationScope::Prohibited("documentation address"),
    ),
    special(
        Ipv6Addr::new(0x2001, 0x10, 0, 0, 0, 0, 0, 0),
        28,
        DestinationScope::Prohibited("deprecated ORCHID address"),
    ),
    special(
        Ipv6Addr::new(0x2001, 0x20, 0, 0, 0, 0, 0, 0),
        28,
        DestinationScope::Prohibited("ORCHIDv2 address"),
    ),
    special(
        Ipv6Addr::new(0x2001, 0x30, 0, 0, 0, 0, 0, 0),
        28,
        DestinationScope::Prohibited("DETs address"),
    ),
    special(
        Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 0),
        23,
        DestinationScope::Prohibited("IETF protocol-assignment address"),
    ),
    special(
        Ipv6Addr::new(0x3fff, 0, 0, 0, 0, 0, 0, 0),
        20,
        DestinationScope::Prohibited("documentation address"),
    ),
    special(
        Ipv6Addr::new(0x2002, 0, 0, 0, 0, 0, 0, 0),
        16,
        DestinationScope::Prohibited("6to4 address"),
    ),
    special(
        Ipv6Addr::new(0x5f00, 0, 0, 0, 0, 0, 0, 0),
        16,
        DestinationScope::Prohibited("SRv6 SID address"),
    ),
    special(
        Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0),
        10,
        DestinationScope::Prohibited("link-local address"),
    ),
    special(
        Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 0),
        7,
        DestinationScope::Private,
    ),
];

pub(crate) fn classify_destination(ip: IpAddr) -> DestinationScope {
    match ip {
        IpAddr::V4(ip) => classify_ipv4(ip),
        IpAddr::V6(ip) if ip.is_unspecified() || ip.is_loopback() => classify_ipv6(ip),
        IpAddr::V6(ip) => ip
            .to_ipv4()
            .map_or_else(|| classify_ipv6(ip), classify_ipv4),
    }
}

fn classify_ipv4(ip: Ipv4Addr) -> DestinationScope {
    let [a, b, c, _] = ip.octets();
    if ip.is_unspecified() || a == 0 {
        return DestinationScope::Prohibited("unspecified address");
    }
    if ip.is_loopback() {
        return DestinationScope::Loopback;
    }
    if ip.is_link_local() {
        return DestinationScope::Prohibited("link-local or cloud metadata address");
    }
    if ip.is_multicast() {
        return DestinationScope::Prohibited("multicast address");
    }
    if ip == Ipv4Addr::BROADCAST {
        return DestinationScope::Prohibited("broadcast address");
    }
    if ip.is_private() {
        return DestinationScope::Private;
    }

    let shared = a == 100 && (64..=127).contains(&b);
    let protocol_assignment = a == 192 && b == 0 && c == 0;
    let deprecated_relay = a == 192 && b == 88 && c == 99;
    let documentation = (a == 192 && b == 0 && c == 2)
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113);
    let benchmarking = a == 198 && matches!(b, 18 | 19);
    if shared
        || protocol_assignment
        || deprecated_relay
        || documentation
        || benchmarking
        || a >= 240
    {
        return DestinationScope::Prohibited("non-public special-purpose address");
    }
    DestinationScope::Public
}

fn classify_ipv6(ip: Ipv6Addr) -> DestinationScope {
    if ip == Ipv6Addr::new(0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0x0254)
        || ip == Ipv6Addr::new(0xfd20, 0x00ce, 0, 0, 0, 0, 0, 0x0254)
    {
        return DestinationScope::Prohibited("cloud metadata address");
    }
    if ip.is_multicast() {
        return DestinationScope::Prohibited("multicast address");
    }
    if (ip.segments()[0] & 0xffc0) == 0xfec0 {
        return DestinationScope::Prohibited("deprecated site-local address");
    }
    IANA_IPV6_SPECIAL_RANGES
        .iter()
        .find(|range| range.contains(ip))
        .map_or(DestinationScope::Public, |range| range.scope)
}

pub(crate) fn is_cloud_metadata_hostname(host: &str) -> bool {
    let host = host.trim_end_matches('.');
    [
        "metadata.google.internal",
        "metadata.goog",
        "instance-data.ec2.internal",
    ]
    .iter()
    .any(|candidate| host.eq_ignore_ascii_case(candidate))
}
