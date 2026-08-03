# ADR 0174: Family-Preserving TCP Endpoint Observation

- Status: Accepted
- Date: 2026-08-03

## Decision

TCP streams expose their local and peer endpoint as bounded typed scalar facts:
address family (`4` or `6`), one IPv4 network-order word or four IPv6
network-order words, numeric IPv6 scope ID, and peer port. The existing
`streamLocalPort` remains the local-port operation.

Word inspection is optional for an invalid word index or mismatched family.
Family, port, and scope inspection trap only for an invalid stream capability.
No text formatting, DNS lookup, interface-name lookup, or dynamic socket-address
object crosses the runtime boundary.

Native ABI 1.38 uses one closed endpoint-part adapter. Pop APIs remain named and
typed; callers can construct the existing canonical `Ipv4SocketAddress`,
`Ipv6SocketAddress`, and scoped values without losing family or scope.
