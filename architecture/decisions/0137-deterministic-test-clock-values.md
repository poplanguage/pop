# ADR 0137: Deterministic Test Clock Values

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0030, 0031, 0032, 0110, 0129, and 0136
- Supersedes: the planned-only deterministic test-clock slice

## Context

Deadlines and timeout logic need deterministic tests before host clocks, task
suspension, and scheduler timers are exposed through PLRI. Reusing wall-clock
timestamps for elapsed time permits clock corrections to reorder deadlines.
Reading ambient time also makes portable library tests nondeterministic.

The canonical `Duration` value already supplies exact signed seconds and
nanoseconds. Ordinary records and an explicitly passed mutable class can model a
bounded monotonic test timeline without compiler recognition, a runtime clock,
or dynamic dispatch.

## Decision

`Pop.Time.Instant` is an ordinary public record with nonnegative `seconds` and
`nanoseconds`. Valid constructed instants satisfy
`0 <= seconds <= 2,147,483,646` and
`0 <= nanoseconds < 1,000,000,000`. The upper bound provides an explicit,
portable test-time budget and permits checked addition with ordinary `Int`
operations.

`Pop.Time.Deadline` is an ordinary public record containing one `Instant`.
`Pop.Time.TestClock` is an explicitly constructed mutable class containing only
its private current instant. It is deterministic, monotonic, not thread-safe,
and never reads ambient time.

The first API is:

- `instant(Int, Int) -> Instant?`, rejecting invalid fields;
- `testClock(Instant) -> TestClock?`, rejecting a manually invalid instant;
- `now(TestClock) -> Instant`, returning an immutable snapshot;
- `advance(TestClock, Duration) -> Boolean`, advancing exactly and returning
  false without mutation for a negative duration, invalid clock state, or
  overflow beyond the test-time budget;
- `deadlineAfter(TestClock, Duration) -> Deadline?`, returning nil for the same
  invalid or overflowing inputs without mutating the clock; and
- `isExpired(TestClock, Deadline) -> Boolean`, using inclusive chronological
  comparison so a deadline expires exactly when the clock reaches it.

Nanosecond addition normalizes with quotient/remainder and checks remaining
seconds before addition. No operation subtracts instants or sleeps.

This test clock is deliberately named `TestClock`; it is not the public host
clock abstraction. Later PLRI contracts must expose monotonic elapsed-time and
wall/calendar time as distinct typed capabilities. Later timers and task
suspension consume those capabilities and keep scheduler policy explicit.

The implementation is ordinary Pop source with no PLRI call, ambient time,
runtime reflection, dynamic lookup, compiler special case, or backend-specific
HIR/MIR operation.

## Consequences

- Deadline and timeout code can be tested deterministically before scheduler
  suspension exists.
- Rejected movement is observable without exceptions and preserves clock state.
- Wall-clock timestamps cannot be accidentally substituted for monotonic
  instants.
- The bounded timeline is a test facility, not a host uptime representation.

## Required conformance

- minimum, normalized carry, exact-boundary, and maximum valid instants construct
  successfully while invalid seconds/nanoseconds are rejected;
- test-clock construction validates manually initialized records;
- `now` returns the exact current snapshot;
- positive and zero advancement preserve canonical fields, while negative and
  overflowing advancement fail without mutation;
- deadlines do not mutate the clock and expire inclusively at their instant;
- checked documentation and the frozen API baseline include three types and six
  functions;
- the same ordinary source reaches verified HIR/MIR and executes on the MIR
  interpreter and LLVM backend; and
- no host clock, wall time, sleep, timer, dynamic value, native duplicate, or
  backend-specific IR is added.

## Documents/components affected

Core catalog, closed decisions, standard implementation plan, API baseline,
ordinary `Pop.Standard` source, documentation checks, and interpreter/LLVM
conformance.
