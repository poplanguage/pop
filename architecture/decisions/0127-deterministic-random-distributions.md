# ADR 0127: Deterministic Random Distributions

- Status: accepted
- Date: 2026-07-28
- Depends on: ADRs 0032, 0051, 0110, and 0126
- Supersedes: none

## Decision

`Pop.Random` appends:

```luau
public function Random.nextInt(
    state: Random.State,
    lowerInclusive: Int,
    upperExclusive: Int,
): Int?
public function Random.nextFloat(state: Random.State): Float64
public function Random.chance(
    state: Random.State,
    probability: Float64,
): Boolean?
```

`nextInt` returns an unbiased integer in `[lowerInclusive, upperExclusive)`.
It returns absence without consuming state when the interval is empty, reversed,
or wider than 4,611,686,009,837,453,316 values. That exact limit is the square
of ADR 0126's generator period. Width calculation covers the complete signed
`Int` domain without overflowing.

Bounds through 2,147,483,646 use one generator value. Larger supported bounds
use two values as one base-2,147,483,646 sample. Rejection sampling removes
modulo bias in both cases.

`nextFloat` consumes one value and returns a deterministic `Float64` in `[0, 1)`
by dividing the zero-based generator value by 2,147,483,646.

`chance` rejects NaN and probabilities outside `[0, 1]` without consuming
state. Exact zero returns false and exact one returns true without consuming
state. Other probabilities consume one `nextFloat` sample.

All operations are ordinary typed Pop and allocate nothing. They retain the
noncryptographic, explicit-state, no-global-generator boundary from ADR 0126.
Normal, exponential, weighted, and other specialized distributions remain
later contracts.

## Required conformance

- empty, reversed, singleton-width, negative-only, positive-only, and
  zero-crossing integer intervals;
- one-sample and two-sample frozen results, rejection behavior, complete `Int`
  width rejection, and no-consumption rejection paths;
- unit-float lower/upper bounds and frozen state consumption;
- chance zero, one, interior, NaN, negative, and above-one behavior;
- interpreter and real LLVM execution agree;
- checked public docs and reference metadata expose exactly three new
  identities; and
- architecture tests reject modulo-only bounded selection and compiler/runtime
  name recognition.
