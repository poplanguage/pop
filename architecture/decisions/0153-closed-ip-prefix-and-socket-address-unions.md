# ADR 0153: Closed IP Prefix and Socket Address Unions

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0021, 0144, 0151, and 0152
- Supersedes: the planned-only family-neutral prefix and endpoint values

## Context

Routing policy and transport configuration need family-neutral values without
losing the distinct IPv4/IPv6 prefix and endpoint representations.

## Decision

`Pop.Net.Prefix` is a closed union of `Ipv4(Ipv4Prefix)` and
`Ipv6(Ipv6Prefix)`. `networkAddress` returns the exact family-preserving
`Address`. `containsAddress` returns false for a family mismatch and otherwise
uses the selected family's prefix containment.

`Pop.Net.SocketAddress` is a closed union of `Ipv4(Ipv4SocketAddress)` and
`Ipv6(Ipv6SocketAddress)`. `parseSocketAddress` selects IPv6 only for bracketed
input and otherwise delegates to strict canonical IPv4 endpoint parsing.
`formatSocketAddress` exhaustively preserves the selected canonical family
spelling.

No implicit conversion, mapped-address normalization, open registry, host
query, socket authority, service-name lookup, or dynamic representation is
added. Scoped interface identities, DNS, transports, and I/O remain later.

## Required conformance

Both cases, family-mismatch containment, canonical endpoint parsing/formatting,
alternate-text rejection, exhaustive matching, API/docs, and MIR/LLVM
execution must agree without host, native, dynamic, or transport operations.

## Consequences

Routing and endpoint configuration can be family-neutral while every operation
remains statically exhaustive over exact family values.
