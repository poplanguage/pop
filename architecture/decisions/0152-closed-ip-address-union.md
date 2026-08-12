# ADR 0152: Closed IP Address Union

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0021, 0129, 0143, and 0150
- Supersedes: the planned-only general IP address value

## Context

DNS answers, interface facts, and family-neutral policy need to carry either an
IPv4 or IPv6 address without String identity, dynamic lookup, or merging their
different representations.

## Decision

`Pop.Net.Address` is a closed tagged union with exactly:

- `Address.Ipv4(value: Ipv4Address)`; and
- `Address.Ipv6(value: Ipv6Address)`.

`parseAddress` tries the strict canonical IPv4 parser and then the strict
canonical IPv6 parser. `formatAddress` exhaustively dispatches to the matching
canonical formatter. `isAddressLoopback` preserves each family's exact
loopback definition. `isAddressUnspecified` recognizes `0.0.0.0` and `::`
without treating private, link-local, multicast, or mapped values as
unspecified.

There is no implicit family conversion, IPv4-mapped normalization, ordering,
hashing, universal table representation, String-backed identity, or open case
registry. Prefix and socket-address unions, scoped interface identities, DNS,
interfaces, transports, and I/O remain separate contracts.

## Required conformance

Tests cover both cases, canonical parsing/formatting, family-preserving
classification, rejection of alternate text, exhaustive matching, API/docs,
and MIR/LLVM agreement without dynamic, native, host, or transport operations.

## Consequences

Later network facts can carry one static family-neutral address while callers
retain exhaustive access to the exact IPv4 or IPv6 value.
