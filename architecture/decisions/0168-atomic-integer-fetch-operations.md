# ADR 0168: Atomic Integer Fetch Operations

- Status: accepted
- Date: 2026-08-03
- Depends on: ADRs 0156, 0157, 0160, and 0165

`Atomic.Int` adds allocation-free `fetchAddInt`, `fetchSubtractInt`,
`fetchAndInt`, `fetchOrInt`, and `fetchXorInt`. Each accepts the existing closed
`Atomic.ReadModifyWriteOrder` value and returns the exact integer observed
before the update. Signed integer arithmetic follows the host atomic
two's-complement wrapping operation; it never introduces a dynamic overflow
mode or a managed allocation.

The operations remain compiler-known standard calls, backend-neutral runtime
operations, and closed ABI 1.30 symbols. MIR interpretation and native-linked
LLVM must agree on returned prior values and final state. Invalid orders or
stale handles fail closed. Fences and wait/notify remain later work.
