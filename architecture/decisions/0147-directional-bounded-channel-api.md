# ADR 0147: Directional Bounded Channel API

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0068, 0145, and 0146
- Supersedes: the unspecified first public `Channel` signatures

## Context

The accepted concurrency model requires typed directional endpoints and exact
close/backpressure outcomes. Returning booleans, strings, numeric ABI statuses,
or a generic tag/value tuple would hide or erase item/empty/closed and
sent/full/closed distinctions. Public endpoint ownership must also be explicit
without introducing finalizers or implicit reference counting.

## Decision

`Pop.Channel` adds four non-prelude nominal identities:

- `Channel.Sender<T>` and `Channel.Receiver<T>` are distinct opaque endpoint
  directions;
- `Channel.SendOutcome` is the closed sent/full/closed outcome; and
- `Channel.ReceiveOutcome<T>` is the closed item/empty/closed outcome.

The first non-suspending API is:

```text
bounded<T>(UInt64) -> (Channel.Sender<T>, Channel.Receiver<T>)?
trySend<T>(Channel.Sender<T>, T) -> Channel.SendOutcome
tryReceive<T>(Channel.Receiver<T>) -> Channel.ReceiveOutcome<T>
close<T>(Channel.Sender<T>) -> Boolean
closeReceiver<T>(Channel.Receiver<T>) -> Boolean
sendAccepted(Channel.SendOutcome) -> Boolean
sendFull(Channel.SendOutcome) -> Boolean
sendClosed(Channel.SendOutcome) -> Boolean
received<T>(Channel.ReceiveOutcome<T>) -> T?
receiveEmpty<T>(Channel.ReceiveOutcome<T>) -> Boolean
receiveClosed<T>(Channel.ReceiveOutcome<T>) -> Boolean
```

`bounded` returns `nil` only when runtime channel identity/storage cannot be
created. It does not eagerly allocate the complete logical capacity. Capacity
zero creates the accepted rendezvous channel; non-suspending `trySend` reports
full until scheduler pairing exists.

`trySend` preserves the caller's value and reports admission, full capacity, or
closed direction. `tryReceive` reports one FIFO item, temporary emptiness, or
permanent closure after the admitted buffer drains. `received` returns the item
only for the item case. Outcome inspection never accepts or returns an ABI
number or runtime String.

Ordinary copies of one endpoint value alias the same directional endpoint
group; they do not silently retain a new producer/consumer. `close` closes the
sender direction for all aliases and is idempotent. `closeReceiver` closes the
receiver direction, discards queued values through precise cleanup, and is
idempotent. Later explicit endpoint duplication, if needed, uses a separately
named operation and exact lifetime contract.

Compiler-known endpoint and send-outcome values have scalar opaque
representations and are not GC references. A receive outcome is a managed
closed value whose optional item slot has an exact compiler-proven type and
root map. HIR/MIR use typed channel operations; native status numbers never
enter HIR/MIR or source.

Suspending `send`/`receive`, cancellation, scheduler rendezvous/wake queues,
unbounded storage, `Task.select`, and actor mailboxes remain later coordinated
layers over the same admission semantics.

## Required conformance

Tests cover generic endpoint direction mismatch, capacity and allocation
failure, scalar and managed FIFO values, every outcome and inspection, buffer
drain before closed, idempotent directional close, receiver discard/root
cleanup, interpreter/LLVM agreement, verified MIR type/root maps, C capability
rejection, and source-free Standard reference consumption.

## Consequences

Pipelines gain exact typed non-suspending channel operations without dynamic
statuses, ambiguous booleans, finalizers, implicit endpoint ownership, or
actor-isolation claims.
