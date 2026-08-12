# ADR 0170: Explicit IPv4 Transport Endpoints

- Status: accepted
- Date: 2026-08-03
- Depends on: ADRs 0143, 0162, 0163, and 0169

Native ABI 1.34 adds numeric IPv4 address plus port operations for TCP listen,
TCP connect, and UDP bind. The address is the exact network-order `UInt32`
payload used by `Net.Ipv4Address`; no text parsing, DNS lookup, implicit host,
or ambient interface selection occurs at the runtime boundary.

The existing port-only operations remain compatible loopback shorthands and
delegate to the explicit endpoint operations. Every resulting handle preserves
the existing nonblocking, bounded-I/O, exact-count, and fail-closed contracts.

Public adapters must accept canonical typed `Net.Ipv4Address` values. Raw
numeric addresses remain an internal ABI representation, not a replacement
public model. IPv6 uses distinct exact operations so family information is
never inferred or erased.

The first public adapters are:

```text
Net.Tcp.listenAt(Net.Ipv4Address, UInt16) -> Net.Tcp.Listener
Net.Tcp.connectTo(Net.Ipv4Address, UInt16) -> Net.Tcp.Stream
Net.Udp.bindAt(Net.Ipv4Address, UInt16) -> Net.Udp.Socket
```

Their backend adapters extract the compiler-proven address field and call the
ABI 1.34 operations. They perform no dynamic member lookup.
