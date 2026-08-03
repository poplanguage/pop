# ADR 0173: TCP Stream Controls

- Status: Accepted
- Date: 2026-08-03

## Decision

`Net.Tcp.Stream` exposes typed half-close and the portable stream controls needed
by protocol implementations:

```pop
Net.Tcp.shutdownRead(Net.Tcp.Stream) -> Boolean
Net.Tcp.shutdownWrite(Net.Tcp.Stream) -> Boolean
Net.Tcp.setNoDelay(Net.Tcp.Stream, Boolean) -> Boolean
Net.Tcp.noDelay(Net.Tcp.Stream) -> Boolean
Net.Tcp.setHopLimit(Net.Tcp.Stream, UInt32) -> Boolean
Net.Tcp.hopLimit(Net.Tcp.Stream) -> UInt32
```

Half-close preserves ownership of the stream handle. Setters return whether the
host accepted the operation. Inspection traps only for an invalid or unavailable
stream capability. Hop-limit names the protocol-neutral public concept; native
IPv4 TTL and IPv6 unicast-hop controls remain platform implementation details.

Native ABI 1.37 carries closed scalar adapters for these operations. MIR
interpreter and LLVM lowering preserve the same results without adding dynamic
socket-option lookup.
