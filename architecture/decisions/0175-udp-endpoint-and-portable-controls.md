# ADR 0175: UDP Endpoint and Portable Controls

- Status: Accepted
- Date: 2026-08-03

## Decision

`Net.Udp.Socket` exposes family-preserving local address words and numeric scope,
broadcast and hop-limit configuration, plus explicit IPv4 multicast membership.
All options are named typed functions; there is no string-selected option API.

Multicast join and leave require both a multicast group and an explicit local
IPv4 interface address. Invalid family word indices return absence. Inspection
traps only for an invalid socket capability, while setters and membership
changes report host acceptance as `Boolean`.

Native ABI 1.39 carries closed scalar adapters. MIR interpreter and LLVM use the
same network-order words and preserve explicit ambient-I/O effects.
