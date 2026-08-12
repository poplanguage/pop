# ADR 0144: IPv4 Prefix and Socket Address Values

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0110, 0129, and 0143
- Supersedes: the planned-only first prefix and socket-address value slice

## Context

Access rules, routes, listeners, and connectors need to distinguish an IP
address, a network prefix, and an address-plus-port before socket authority
exists. String concatenation loses those identities and encourages unchecked
port and prefix arithmetic.

## Decision

`Pop.Net.Ipv4Prefix` contains an `Ipv4Address` and prefix length 0 through 32.
Construction validates the length but preserves the supplied host bits.
`networkIpv4` returns the canonical network address and `containsIpv4` compares
only prefix bits using overflow-safe integer division.

`Pop.Net.Ipv4SocketAddress` contains an `Ipv4Address` and `UInt16` port.
Construction accepts every port, including zero for explicit bind allocation.
Strict text parsing accepts canonical IPv4 text, one colon, and a canonical
decimal port 0 through 65,535 without leading zeroes. Formatting is canonical.

The API is `ipv4Prefix`, `networkIpv4`, `containsIpv4`, `ipv4Socket`,
`parseIpv4Socket`, and `formatIpv4Socket`.

IPv6 prefixes/socket spelling, service names, DNS, socket handles, bind,
connect, listen, and I/O remain later contracts.

## Required conformance

Prefix lengths 0, 32, and invalid neighbors; host-bit normalization;
containment boundaries; all port boundaries; malformed/ambiguous text; checked
docs/API; and MIR/LLVM execution must agree without host or dynamic operations.

## Consequences

Network APIs gain typed routing and endpoint values without acquiring a socket.
