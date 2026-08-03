# ADR 0176: Unix-Domain Stream Transport

- Status: Accepted
- Date: 2026-08-03

## Decision

`Net.Unix` exposes opaque `Listener` and `Stream` capabilities with explicit
filesystem paths, nonblocking accept, owning-`Bytes` send, reusable-buffer
receive, half-close, and separate listener/stream close operations.

Transfers reuse `Net.Transfer`, including progress, would-block, closed, and
exact byte-count inspection. Paths are explicit UTF-8 strings at this platform
boundary; the runtime rejects embedded zero bytes and never deletes or replaces
an existing path. The application owns socket-path cleanup.

Native ABI 1.40 implements the platform adapter. MIR interpreter and LLVM retain
the same typed handles and transfer encoding. Unsupported targets fail by target
capability rather than emulating Unix sockets over TCP.
