# ADR 0155: Immutable Network Interface and Route Facts

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0021, 0129, 0152, 0153, and 0154
- Supersedes: the planned-only first interface and route fact slice

## Context

Interface queries, route snapshots, diagnostics, and test fixtures need typed
facts before platform enumeration exists. A universal table would erase family
constraints and permit invalid IPv4-prefix/IPv6-next-hop combinations.

## Decision

`NetworkInterface` is an immutable fact containing nonzero `InterfaceId`, an
exact nonempty host-supplied name, up and loopback flags, and a `UInt32` maximum
transmission unit. The constructor validates only the nonempty name; it does
not invent platform policy for the other facts.

`InterfaceAddress` associates one `InterfaceId` with one closed `Prefix`.

`Route` is a closed union with `Ipv4OnLink`, `Ipv4Via`, `Ipv6OnLink`, and
`Ipv6Via` cases. Every case contains a same-family destination prefix,
`InterfaceId`, and `UInt32` metric; `Via` cases additionally contain a concrete
same-family next hop. Thus absence is nominal and a cross-family next hop is
unrepresentable.
Separate `OnLink` and `Via` constructors plus exhaustive accessors expose
destination, next hop, interface, and metric while preserving family identity.

These are caller-supplied values. Interface enumeration, route acquisition,
change observation, platform flags, DNS, sockets, transports, and I/O require
later capability and PLRI contracts.

## Required conformance

Tests cover name validation, exact facts, both route cases, absent/present next
hops, family preservation, exhaustive accessors, API/docs, and MIR/LLVM
agreement without host, native, dynamic, or transport operations.

## Consequences

Platform adapters and deterministic tests gain one static interchange model
without granting ambient network inspection.
