# ADR 0160: Atomic Operation Vocabulary

- Status: accepted
- Date: 2026-08-03

The backend-neutral `RuntimeOperation` vocabulary includes separate typed
Atomic integer and Boolean create/load/store/swap/strong-compare-exchange
operations plus one typed release operation. Native symbol resolution maps
each operation to the closed ABI symbols from ADR 0157. HIR/MIR lowering and
public compiler call checking remain subsequent implementation layers.

The trusted Standard bootstrap also reserves nominal `Atomic.Int` and
`Atomic.Boolean` identities. They are non-prelude types; constructors and
methods must remain explicitly typed.

Atomic lowering requires both the `AtomicOperations` runtime contract and the
backend-neutral `Atomics` target capability. Runtime-profile selection rejects
the program before emission when either side is unavailable.
