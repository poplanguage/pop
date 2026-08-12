# ADR 0150: Canonical IPv6 Address Values

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0030, 0031, 0032, 0110, 0129, and 0143
- Supersedes: the planned-only first pure IPv6 address slice

## Context

IPv6 sockets, prefixes, routes, DNS answers, and access rules require an exact
128-bit address identity. Pop Lang has no `UInt128`, and a String-backed value
would permit multiple spellings to masquerade as semantic identity.

## Decision

`Pop.Net.Ipv6Address` is an ordinary record containing four network-order
`UInt32` words. Its first API is:

- `ipv6(UInt16, UInt16, UInt16, UInt16, UInt16, UInt16, UInt16, UInt16)`;
- `parseIpv6(String) -> Ipv6Address?`;
- `formatIpv6(Ipv6Address) -> String`;
- `ipv6Segment(Ipv6Address, Int) -> UInt16?`, with one-based indexing;
- `isIpv6Loopback(Ipv6Address) -> Boolean`; and
- `isIpv6Unspecified(Ipv6Address) -> Boolean`.

Canonical text is lowercase hexadecimal without leading zeroes. Formatting
compresses the first longest run of at least two zero segments and never
compresses one isolated zero. Parsing accepts only that canonical spelling by
reformatting the decoded value and requiring exact equality.

The first parser rejects uppercase, leading zeroes, multiple or unnecessary
compression, fewer or more than eight expanded segments, non-ASCII text,
embedded dotted IPv4, zone identifiers, prefixes, ports, and whitespace.
Loopback is exactly `::1`; unspecified is exactly `::`.

IPv6 prefixes/socket addresses, scoped interface identities, IPv4-mapped
classification, multicast scopes, DNS, interfaces, sockets, and I/O remain
later typed contracts. The implementation is ordinary Pop with no host query
or PLRI operation.

## Required conformance

Tests cover constructor/segment boundaries, uncompressed values, leading,
interior, and trailing compression, longest-run ties, loopback/unspecified
classification, malformed/noncanonical negatives, checked docs/API baseline,
and MIR-interpreter/LLVM native execution. No String-backed identity, host
network operation, dynamic value, native duplicate, or backend-specific IR is
introduced.

## Consequences

Network APIs gain one deterministic IPv6 identity and interchange spelling
without acquiring interface, DNS, or socket authority.
