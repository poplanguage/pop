# ADR 0143: Bounded IPv4 Address Values

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0030, 0031, 0032, 0110, and 0129
- Supersedes: the planned-only first pure `Net` address slice

## Context

Sockets, DNS results, URI host policy, access rules, and test transports need a
typed IP identity before any network capability is exposed. Raw strings permit
ambiguous leading-zero spelling and defer validation to host APIs.

## Decision

`Pop.Net.Ipv4Address` is an ordinary record containing one exact network-order
`UInt32`. The first API is:

- `ipv4(Byte, Byte, Byte, Byte) -> Ipv4Address`;
- `parseIpv4(String) -> Ipv4Address?`;
- `formatIpv4(Ipv4Address) -> String`;
- `ipv4Octet(Ipv4Address, Int) -> Byte?`, with one-based indexing;
- `isIpv4Loopback(Ipv4Address) -> Boolean`; and
- `isIpv4Private(Ipv4Address) -> Boolean`.

Parsing accepts exactly four decimal octets, rejects empty components,
non-ASCII, signs, surrounding whitespace, values over 255, and multi-digit
leading zeroes. Formatting is canonical dotted decimal. Loopback is 127/8.
Private space is exactly 10/8, 172.16/12, and 192.168/16; link-local,
documentation, multicast, broadcast, and unspecified addresses are not called
private by this predicate.

IPv6, prefixes, socket addresses/ports, interface and route facts, DNS, sockets,
and all I/O remain later typed contracts. The implementation is ordinary Pop
with no host query or PLRI operation.

## Required conformance

- all octet boundaries and canonical spelling round-trip;
- malformed, leading-zero, and overflow inputs reject;
- one-based octet extraction and named ranges cover positive/negative edges;
- checked docs, API baseline, MIR interpreter, and LLVM agree; and
- no string-backed address, DNS, socket, dynamic value, native duplicate, or
  backend-specific IR is introduced.

## Consequences

Network-facing APIs gain a canonical static address identity before acquiring
network authority.
