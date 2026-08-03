# ADR 0167: Typed Loopback Net Transport Adapters

- Status: accepted
- Date: 2026-08-03
- Depends on: ADRs 0162, 0163, and 0164

`Pop.Net.Tcp` and `Pop.Net.Udp` expose the first compiler-known transport
adapters over the closed native runtime operations. Listener, stream, and
socket values are distinct opaque nominal handles. `Net.SocketIoOutcome` is a
closed progress, would-block, or closed result. TCP receive additionally uses
`Net.Tcp.Receive`, and a successful UDP receive yields `Net.Udp.Datagram` with
an exact numeric IPv4 source address and port.

The executable bootstrap surface is:

```text
Net.Tcp.listen(UInt16) -> Net.Tcp.Listener
Net.Tcp.listenerLocalPort(Net.Tcp.Listener) -> UInt16
Net.Tcp.streamLocalPort(Net.Tcp.Stream) -> UInt16
Net.Tcp.connect(UInt16) -> Net.Tcp.Stream
Net.Tcp.accept(Net.Tcp.Listener) -> Net.Tcp.Stream?
Net.Tcp.sendByte(Net.Tcp.Stream, Byte) -> Net.SocketIoOutcome
Net.Tcp.receiveByte(Net.Tcp.Stream) -> Net.Tcp.Receive
Net.Tcp.closeListener(Net.Tcp.Listener) -> Boolean
Net.Tcp.closeStream(Net.Tcp.Stream) -> Boolean
Net.Udp.bind(UInt16) -> Net.Udp.Socket
Net.Udp.localPort(Net.Udp.Socket) -> UInt16
Net.Udp.sendByteTo(Net.Udp.Socket, UInt32, UInt16, Byte) -> Net.SocketIoOutcome
Net.Udp.receiveByte(Net.Udp.Socket) -> Net.Udp.Datagram?
Net.Udp.close(Net.Udp.Socket) -> Boolean
```

Pure inspection functions expose socket outcomes, TCP received bytes, and UDP
datagram byte/address/port fields. Native failure traps because it represents a
rejected typed handle or runtime invariant; would-block and closed remain
ordinary values. Bind port zero requests an ephemeral loopback port. Every I/O
operation is nonblocking after construction and carries `AmbientIo`; no DNS,
environment lookup, implicit host, or string-selected operation enters the
pipeline.

The one-byte operations are the executable foundation for later caller-owned
buffer, deadline, cancellation, half-close, socket-option, non-loopback
capability, Unix-domain, TLS, and QUIC layers. They do not claim those broader
transport layers are complete.
