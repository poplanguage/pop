# ADR 0165: Typed Atomic Standard API

- Status: accepted
- Date: 2026-08-03

`Pop.Atomic` exposes compiler-known non-prelude `Int`, `Boolean`, `LoadOrder`,
`StoreOrder`, and `ReadModifyWriteOrder` values. Closed constructor functions
create every legal load, store, and read-modify-write order. Typed integer and
Boolean functions provide creation, load, store, swap, and release without
overload ambiguity or dynamic dispatch:

```text
Atomic.int(Int) -> Atomic.Int
Atomic.boolean(Boolean) -> Atomic.Boolean
Atomic.loadInt(Atomic.Int, Atomic.LoadOrder) -> Int
Atomic.loadBoolean(Atomic.Boolean, Atomic.LoadOrder) -> Boolean
Atomic.storeInt(Atomic.Int, Int, Atomic.StoreOrder) -> Boolean
Atomic.storeBoolean(Atomic.Boolean, Boolean, Atomic.StoreOrder) -> Boolean
Atomic.swapInt(Atomic.Int, Int, Atomic.ReadModifyWriteOrder) -> Int
Atomic.swapBoolean(Atomic.Boolean, Boolean, Atomic.ReadModifyWriteOrder) -> Boolean
Atomic.compareExchangeInt(Atomic.Int, Int, Int, Atomic.ReadModifyWriteOrder, Atomic.LoadOrder) -> Int
Atomic.compareExchangeBoolean(Atomic.Boolean, Boolean, Boolean, Atomic.ReadModifyWriteOrder, Atomic.LoadOrder) -> Boolean
Atomic.releaseInt(Atomic.Int) -> Boolean
Atomic.releaseBoolean(Atomic.Boolean) -> Boolean
```

Order constructors are pure. State operations carry `Synchronizes`; loads,
swaps, and compare-exchange additionally carry `MayTrap` because a rejected
native handle or invalid success/failure order pair is a runtime invariant
failure. Strong compare-exchange returns the exact previously observed scalar.
The caller determines whether exchange occurred by comparing that value with
the expected scalar, preserving all information without allocating a result
object or reducing the operation to a Boolean-only result.

The trusted function metadata drives source checking, HIR verification, MIR
verification, and effect lowering. LLVM and MIR-interpreter adapters consume
the existing typed RuntimeOperation ABI; no generic runtime-call opcode or
string-selected operation is introduced.
