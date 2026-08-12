# ADR 0154: Numeric Interface Identities and Scoped IPv6 Values

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0110, 0129, 0150, and 0153
- Supersedes: the planned-only first scoped network-interface identity slice

## Context

Link-scoped IPv6 values need an explicit interface identity. Ambient name
resolution would be nondeterministic and String-only zones would conflate
portable values with host interface queries.

## Decision

`Pop.Net.InterfaceId` contains one nonzero `UInt32` numeric interface index.
`interfaceId` rejects zero. The value is an explicit identity supplied by a
caller or later interface capability; constructing it never queries the host.

`Pop.Net.ScopedIpv6Address` contains an `Ipv6Address` and `InterfaceId`.
`scopedIpv6` preserves both values. Canonical text is exactly
`canonical-ipv6%decimal-index`, using no leading zeroes and the complete range
1 through 4,294,967,295. Parsing rejects names, zero, overflow, whitespace,
multiple percent signs, and every noncanonical IPv6 spelling.

Scope attachment is explicit and does not claim that a particular address
requires or permits a scope. That policy belongs to the later interface and
socket operation accepting the value. Interface enumeration/names, endpoint
zones, routes, DNS, transports, and I/O remain later contracts.

## Required conformance

Tests cover index boundaries, canonical scoped text, malformed and alternate
text, exact value preservation, API/docs, and MIR/LLVM execution without host,
native, dynamic, interface lookup, or transport behavior.

## Consequences

Later network capabilities can carry deterministic interface scope without
ambient lookup or weakening IPv6 address identity.
