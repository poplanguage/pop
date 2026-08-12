# ADR 0126: Deterministic Random State

- Status: accepted
- Date: 2026-07-28
- Depends on: ADRs 0032, 0051, 0058, 0097, 0110, and 0117
- Supersedes: none

## Decision

`Pop.Random` provides one explicit mutable generator:

```luau
public class State
    private value: UInt32
end

public function Random.seed(value: UInt32): Random.State
public function Random.next(state: Random.State): UInt32
public function Random.fill(
    state: Random.State,
    output: Bytes.Buffer,
    count: Int,
): Boolean
public function Random.shuffle<T>(
    state: Random.State,
    values: {T},
): Boolean
```

`seed` is deterministic and is the only allocation in the generator lifecycle.
Every `UInt32` seed is accepted. It is reduced modulo 2,147,483,647, and zero
normalizes to one. `next` mutates the explicit state and returns the next value
from the Park-Miller MINSTD stream using multiplier 16,807 and Schrage's
overflow-free decomposition. Results are in 1 through 2,147,483,646.

The algorithm and constants are part of the reproducibility contract. A later
generator uses a separately named state type and never silently changes this
stream.

`fill` appends exactly `count` unbiased bytes to `output`. It uses rejection
sampling over the generator period and returns false without mutation for a
negative count. Zero succeeds without mutation. Buffer capacity growth remains
the buffer's private policy.

`shuffle` performs an in-place Fisher-Yates permutation. It uses rejection
sampling, one generator result for bounds through 2,147,483,646, and two
base-2,147,483,646 results for larger bounds. It returns false without mutating
the state or array only when the array length exceeds the square of that base.
Zero- and one-item arrays succeed without consuming state.

These APIs are deterministic pseudo-random facilities, not cryptography.
There is no ambient/global generator, runtime string selection, clock,
environment, or implicit entropy. Nondeterministic seeding remains a later
explicit entropy-capability contract. Cryptographic randomness remains in
`Crypto`.

## Required conformance

- seed zero, one, the modulus, and `UInt32` maximum normalize deterministically;
- the first values and a long stream checkpoint match the frozen MINSTD stream;
- `next` allocates nothing and all intermediate signed arithmetic stays in
  `Int`;
- `fill` covers negative, zero, growth, exact byte count, rejection, and a
  frozen byte sequence;
- `shuffle` covers empty, singleton, repeated values, frozen permutations,
  state consumption, and generic element types;
- interpreter and real LLVM execution agree;
- checked public docs and reference metadata expose exactly `State`, `seed`,
  `next`, `fill`, and `shuffle`;
- a target-labelled throughput benchmark covers `next`, `fill`, and `shuffle`;
  and
- architecture tests reject global state, native/compiler name recognition,
  cryptographic claims, and algorithm-constant drift.
