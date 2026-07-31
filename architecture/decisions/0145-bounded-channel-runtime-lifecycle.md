# ADR 0145: Bounded Channel Runtime Lifecycle

- Status: accepted
- Date: 2026-07-31
- Depends on: ADR 0068
- Supersedes: the unspecified private bounded-channel lifecycle

## Context

ADR 0068 requires exact directional channels, FIFO buffering, backpressure,
closure, cancellation, and precise GC behavior. Interpreter and native
schedulers need one backend-neutral admission and endpoint-lifetime contract
before public suspension operations can share observable semantics.

## Decision

PLRI defines a typed `ChannelLifecycle<T>` identified by `ChannelId`. A bounded
channel starts with one sender and one receiver, preserves admitted values in
FIFO order, and never owns more than its declared capacity. Capacity zero is an
explicit rendezvous channel: non-suspending send reports full until scheduler
waiters are paired.

Non-suspending send returns the original exact `T` when the channel is full or
closed. Non-suspending receive returns exactly item, empty, or closed. Closing
the sender direction rejects later sends but drains admitted values before
receive reports closed. Releasing the last sender has the same close effect.
Releasing the last receiver closes admission and returns all buffered values to
the runtime so their precise roots can be released. Endpoint retain after its
direction closes fails closed.

The lifecycle contains no native symbol, backend opcode, erased payload, string
status, global registry, scheduler policy, or actor-isolation behavior.
Scheduler wait queues, cancellation arbitration, wake order, native ABI
storage, MIR operations, public `Channel<T>` signatures, unbounded channels,
and typed selection build on this contract in later coordinated slices.

## Required conformance

Tests cover FIFO admission, capacity backpressure without payload loss, sender
close and buffer drain, endpoint retain/release, last-receiver payload cleanup,
closed-direction rejection, and zero-capacity rendezvous readiness. Interpreter
and native channel operations must later consume the same lifecycle outcomes.

## Consequences

Channel buffering and closure now have one backend-neutral semantic source.
Adding suspension cannot redefine admission, closure, or payload-root cleanup.
