# ADR 0171: Explicit IPv6 Transport Endpoints

- Status: accepted
- Date: 2026-08-03
- Depends on: ADRs 0150, 0154, 0162, 0163, and 0170

Native ABI 1.35 adds exact IPv6 TCP listen/connect and UDP bind operations. An
address crosses the ABI as the four network-order `UInt32` words of
`Net.Ipv6Address`; the numeric scope ID is a separate required argument.

Scope zero is the canonical unscoped form. Nonzero scope preserves a
compiler-proven `Net.InterfaceId` for link-local and other scoped endpoints.
The runtime performs no interface-name lookup, DNS resolution, family
inference, or text parsing. Handles retain the established nonblocking,
bounded-I/O, and fail-closed behavior.

Public unscoped adapters overload `Net.Tcp.listenAt`, `Net.Tcp.connectTo`, and
`Net.Udp.bindAt` for `Net.Ipv6Address`. Scoped adapters consume
`Net.ScopedIpv6Address` without flattening it into text.
