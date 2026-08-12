# ADR 0136: Canonical Duration Values

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0030, 0031, 0032, 0110, and 0129
- Supersedes: the planned-only first `Time.Duration` slice

## Context

Deadlines, timers, process limits, retries, scheduling, and protocol timeouts
need one exact portable duration before clock and scheduler capabilities are
introduced. A raw integer leaves its unit ambiguous. Floating-point seconds
lose exact nanoseconds and make ordering depend on rounding.

Ordinary public records can carry an immutable two-part value across Bubbles
without a runtime clock, native layout, or backend-specific operation.

## Decision

`Pop.Time.Duration` is an ordinary public record with signed `seconds` and
nonnegative `nanoseconds`. Canonical values always satisfy
`0 <= nanoseconds < 1,000,000,000`; `seconds` is the mathematical floor of the
complete duration. Thus negative half a second is represented by `seconds = -1`
and `nanoseconds = 500,000,000`.

The first exact API is:

- `fromSeconds(Int) -> Duration`;
- `fromMilliseconds(Int) -> Duration`;
- `fromNanoseconds(Int) -> Duration`;
- `compare(Duration, Duration) -> Int`, returning negative one, zero, or one
  without subtraction overflow;
- `isZero(Duration) -> Boolean`;
- `isNegative(Duration) -> Boolean`;
- `secondsPart(Duration) -> Int`; and
- `nanosecondsPart(Duration) -> Int`.

Millisecond and nanosecond construction uses quotient/remainder decomposition,
then adjusts a negative remainder into the canonical floor representation.
Every `Int` input is accepted without multiplication or negation overflow.

Manual record initializers remain responsible for the field invariant. Public
inspection and comparison are deterministic for every statically typed record,
while constructors are the canonical creation path.

Addition, subtraction, scaling, division, total-unit conversion, parsing,
formatting, rounding, saturation, overflow results, `Instant`, wall time,
calendar values, deadlines, clocks, timers, sleeping, and scheduling remain
focused later contracts. No duration operation reads a clock or suspends.

The implementation is ordinary Pop source with no PLRI call, ambient time,
runtime reflection, dynamic lookup, or backend-specific HIR/MIR operation.

## Consequences

- Timeout-bearing APIs share an exact unit-bearing value before host time is
  available.
- Negative subsecond values retain one canonical representation.
- Later clock and arithmetic contracts can define overflow and capability
  behavior without changing Duration identity.

## Required conformance

- zero, positive, negative, exact-unit, subunit, and complete `Int` boundary
  constructor inputs produce canonical fields;
- negative millisecond/nanosecond remainders normalize by floor rather than
  truncation toward zero;
- comparison covers differing seconds, differing nanos, equality, and extreme
  seconds without subtraction;
- zero/negative inspection and exact part accessors agree with canonical
  fields;
- checked documentation and the frozen API baseline include the record and
  eight functions;
- the same ordinary source reaches verified HIR/MIR and executes on the MIR
  interpreter and LLVM backend; and
- no clock, timer, suspension, floating-point unit, dynamic value, native
  duplicate, or backend-specific IR is added.

## Documents/components affected

Core catalog, essential-library projection, closed decisions, standard
implementation plan, API baseline, ordinary `Pop.Standard` source,
documentation checks, and interpreter/LLVM conformance.
