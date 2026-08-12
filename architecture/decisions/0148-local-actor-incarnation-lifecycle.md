# ADR 0148: Local Actor Incarnation Lifecycle

- Status: accepted
- Date: 2026-07-31
- Depends on: ADR 0068
- Supersedes: the unspecified backend-neutral actor mailbox lifecycle

## Context

The accepted local actor model requires bounded FIFO admission, stale
incarnation detection, and an exit boundary that finishes cleanup before
publishing termination. Reusing `ChannelLifecycle<T>` directly would erase
actor-specific incarnation, isolation, and exit semantics. A runtime registry,
numeric status convention, or dynamically erased message would move those
semantics out of verified backend-neutral contracts.

## Decision

PLRI defines a generic `ActorLifecycle<T>` substrate with:

- a stable local `ActorId` plus monotonically distinct `ActorIncarnation`;
- an exact `ActorReference` containing both identities;
- `Starting`, `Running`, `Stopping(exit)`, and `Exited(exit)` states;
- bounded FIFO admission of an already compiler-checked and copied `T`;
- closed `Full(T)`, `Closed(T)`, and `Stale(T)` admission failures that preserve
  the unconsumed message; and
- separate begin/complete exit transitions.

Only a running actor accepts or dequeues messages. Admission checks the complete
reference before checking capacity. Restart constructs a new lifecycle with a
new incarnation; it never retargets an old reference.

Beginning exit atomically closes admission and returns every queued message in
FIFO order to the owning runtime. The runtime uses those exact values to
release precise roots and copied ownership. It then cancels and joins child
tasks, runs registered cleanup, and completes exit. Terminal publication never
precedes that cleanup.

This substrate does not claim that a value is actor-message-safe, perform the
copy into actor ownership, schedule the actor entry, expose public `Pop.Actor`
signatures, implement replies/monitors/supervision, or define a native ABI.
Those coordinated layers must preserve this lifecycle.

## Required conformance

Tests cover activation, exact-capacity FIFO admission, full-value preservation,
zero capacity, stale references after restart, closed admission during exit,
queued-value return for root cleanup, and single-use exit transitions. The
contract contains no dynamic payload, runtime string dispatch, hash-selected
ordering, or implicit reference retargeting.

## Consequences

Actor runtime work gains a deterministic typed lifecycle that stays distinct
from Channels and distributed actors. Scheduler, compiler copy maps, isolated
GC ownership, native storage, and the public standard-library surface remain
explicit follow-on work rather than hidden behavior.
