# ADR 0166: Typed Local Actor Mailboxes

- Status: accepted
- Date: 2026-08-03
- Depends on: ADRs 0068, 0148, 0149, 0158, and 0161

The first executable public `Actor` surface exposes bounded local mailboxes for
exact integer message types:

```text
Actor.mailbox<T>(UInt64, UInt64, UInt64) -> Actor.Inbox<T>?
Actor.reference(Actor.Inbox<T>) -> Actor.Ref<T>
Actor.trySend(Actor.Ref<T>, T) -> Actor.SendOutcome
Actor.tryReceive(Actor.Inbox<T>) -> T?
Actor.finish(Actor.Inbox<T>) -> Boolean
Actor.release(Actor.Inbox<T>) -> Boolean
Actor.sendAccepted(Actor.SendOutcome) -> Boolean
Actor.sendFull(Actor.SendOutcome) -> Boolean
Actor.sendClosed(Actor.SendOutcome) -> Boolean
Actor.sendStale(Actor.SendOutcome) -> Boolean
```

`mailbox` takes the stable actor identity, incarnation, and capacity, creates
and activates one lifecycle, and returns absence if native creation fails.
`Ref<T>` and `Inbox<T>` remain distinct static capabilities even where the
bootstrap backend represents both with one opaque handle. Admission remains
bounded FIFO and reports full, closed, and stale outcomes without suspension.
`finish` performs completed begin-exit and complete-exit transitions; `release`
invalidates the native handle.

This executable slice admits only integer primitives. They cross the ABI by
value, so no mutable alias or incomplete managed graph can cross the actor
boundary. The compiler, HIR verifier, and MIR verifier all enforce that limit.
Immutable aggregate messages, replies, actor-entry scheduling, supervision,
monitors, restart policies, and suspending operations require the later exact
copy-map and structured-task layers. They must not enter through a dynamic or
unchecked fallback.

`tryReceive` returns absence for both an empty running mailbox and a closed
mailbox. Code that does not own the lifecycle must use send outcomes for remote
state observation; a later suspending receive result may distinguish closure.

