# ADR 0151: IPv6 Prefix and Socket Address Values

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0110, 0129, 0144, and 0150
- Supersedes: the planned-only IPv6 prefix and socket-address value slice

## Context

IPv6 routing rules and endpoints need identities distinct from a plain address.
Prefix arithmetic must cover all 128 bits without `UInt128`, while endpoint text
must distinguish address colons from the numeric port without heuristics.

## Decision

`Pop.Net.Ipv6Prefix` contains an `Ipv6Address` and prefix length 0 through 128.
Construction validates the length while preserving supplied host bits.
`networkIpv6` clears host bits segment by segment, and `containsIpv6` compares
only the selected prefix bits. Both operations use bounded integer arithmetic
over the address's eight network-order `UInt16` segments.

`Pop.Net.Ipv6SocketAddress` contains an `Ipv6Address` and `UInt16` port.
Construction accepts every port, including zero. Canonical text is exactly
`[address]:port`, where `address` is ADR 0150 canonical IPv6 text and `port` is
canonical decimal from 0 through 65,535 without leading zeroes. Unbracketed
addresses, service names, zones, whitespace, and alternate address or port
spellings are rejected.

The API is `ipv6Prefix`, `networkIpv6`, `containsIpv6`, `ipv6Socket`,
`parseIpv6Socket`, and `formatIpv6Socket`.

A closed IPv4-or-IPv6 address union, scoped interface identities, DNS, socket
handles, bind, connect, listen, and I/O remain later contracts.

## Required conformance

Prefix lengths 0, 128, partial segments, and invalid neighbors; host-bit
normalization; containment boundaries; ports 0 and 65,535; malformed or
noncanonical bracketed text; checked docs/API; and MIR/LLVM execution must agree
without host, dynamic, native, or transport operations.

## Consequences

Network APIs gain typed IPv6 routing and endpoint values without acquiring
interface scope or socket authority.
