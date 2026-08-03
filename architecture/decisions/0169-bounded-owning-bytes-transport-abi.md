# ADR 0169: Bounded Owning Bytes Transport ABI

- Status: accepted
- Date: 2026-08-03
- Depends on: ADRs 0117, 0162, 0163, and 0167

Native ABI 1.31 adds bounded TCP and UDP operations over immutable owning
`Bytes`. Sends report the exact transferred count. Receives require a nonzero
caller limit, allocate an exact immutable result only after transport progress,
and report the exact received count. UDP receive additionally returns the
numeric source address and port.

The ABI 1.31 adapters copy between managed immutable storage and the operating-system
call boundary. Managed pointers never escape, and a would-block or closed
result allocates nothing. Invalid handles, invalid byte references, zero
receive limits, and allocation failure fail closed. The existing raw-pointer
ABI remains an internal adapter for compiler-owned temporary storage.

MIR and public `Pop.Net` adapters must preserve the same limits, counts,
transport statuses, and UDP source facts. Borrowed-view sends may optimize
copies later without changing these semantics.

The first public adapters are `Net.Tcp.send(Stream, Bytes)` and
`Net.Tcp.receive(Stream, Bytes.Buffer, UInt64)`. They return `Net.Transfer`,
whose inspection functions expose progress, would-block, closed, and the exact
transferred byte count. Receive appends atomically to the caller's reusable
buffer only on progress.

Native ABI 1.32 adds the bounded TCP receive adapter that appends directly to
the caller's reusable `Bytes.Buffer`. This keeps the backend operation atomic,
avoids an intermediate managed allocation, and preserves the ABI 1.31 status
and exact-count contract.
