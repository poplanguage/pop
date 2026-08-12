# ADR 0157: Typed Atomic Native ABI Handles

- Status: accepted
- Date: 2026-08-03

## Decision

`Pop.Standard` atomic values use opaque, nonzero native handles. The ABI keeps
integer and Boolean state as separate typed operations; it never accepts a
runtime type tag, managed pointer, or dynamic operation selector.

The native boundary exposes create, load, store, swap, compare-exchange, and
release symbols for signed 64-bit integers and Booleans. Ordering arguments are
closed numeric values matching ADR 0156. Invalid handles, output pointers, and
order combinations fail with a closed status and do not mutate state.

Compare-exchange returns the observed previous value through a caller-owned
scalar output and reports success or mismatch separately. Handles are runtime
registry identities, not object addresses; release removes the identity after
the final operation.

## Consequences

ABI tests can validate exact symbols, widths, invalid-input behavior, and
cross-thread acquire/release publication without coupling HIR or MIR to a
backend object. Compiler-known public Atomic lowering remains a separate step.
