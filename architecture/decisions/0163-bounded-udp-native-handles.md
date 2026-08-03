# ADR 0163: Bounded UDP Native Handles

- Status: accepted
- Date: 2026-08-03

The native runtime exposes opaque UDP socket handles bound to explicit IPv4
loopback endpoints. Datagrams use numeric network-order IPv4 addresses and
bounded caller buffers; DNS, environment lookup, implicit routing, and global
transport registries are excluded. Invalid handles, spans, and output slots
fail closed. Deadlines, cancellation, multicast, and public `Net.Udp` wiring
remain later layers.

ABI 1.28 returns a closed failure/progress/would-block status from datagram send
and receive and writes byte counts separately. A zero-length datagram therefore
remains successful progress rather than becoming an ambiguous failure.

The backend-neutral `RuntimeOperation` inventory and native ABI symbol catalog
reserve exact entries for every operation in this slice. Lowering requires the
backend-neutral `Networking` target capability; the supported Linux native
target declares it and freestanding targets do not.
The selected runtime profile must also provide the distinct `NetworkIo`
runtime contract.
