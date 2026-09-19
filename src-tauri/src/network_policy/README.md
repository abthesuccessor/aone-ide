# Network destination policy

This feature owns the transport-independent outbound address policy used by the
HTTP API Explorer and WebSocket console.

- `address.rs` classifies resolved IPv4/IPv6 addresses and recognizes known
  cloud metadata hostnames.
- `tests.rs` is the single regression suite for public, private, loopback,
  metadata, link-local, multicast, documentation, transition, and other
  special-purpose ranges.
- `mod.rs` contains declarations and crate-only reexports; it has no behavior.

Protocol modules still own their own URL rules, DNS timeouts, native consent,
and address pinning. Every resolved address must pass this policy before either
transport connects. Private and loopback addresses are warning scopes; all
other non-public/special scopes covered here are prohibited.

## IPv6 registry snapshot

The compact range table in `address.rs` mirrors all entries in the authoritative
[IANA IPv6 Special-Purpose Address Space registry](https://www.iana.org/assignments/iana-ipv6-special-registry/iana-ipv6-special-registry.xhtml),
last updated 2025-10-09. More-specific entries appear before their containing
range so diagnostics retain the registered purpose. The checked-in prefixes are:

- `::1/128`, `::/128`, `::ffff:0:0/96`
- `64:ff9b::/96`, `64:ff9b:1::/48`
- `100::/64`, `100:0:0:1::/64`
- `2001::/23`, `2001::/32`, `2001:1::1/128`, `2001:1::2/128`,
  `2001:1::3/128`, `2001:2::/48`, `2001:3::/32`, `2001:4:112::/48`,
  `2001:10::/28`, `2001:20::/28`, `2001:30::/28`, `2001:db8::/32`
- `2002::/16`, `2620:4f:8000::/48`, `3fff::/20`, `5f00::/16`
- `fc00::/7`, `fe80::/10`

Loopback and unique-local destinations retain warning scopes. Every other
registered special-purpose range is prohibited, including registry entries
marked globally reachable, because this IDE only treats ordinary global
destinations as public. IPv4-compatible and IPv4-mapped IPv6 literals are
classified from their embedded IPv4 address, preserving public, private,
loopback, and prohibited outcomes. Multicast, deprecated site-local, and known
cloud-metadata addresses are also prohibited outside the registry table.
