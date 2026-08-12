# ADR 0158: Local Actor Native Mailboxes

- Status: accepted
- Date: 2026-08-03

## Decision

The native Actor layer stores one opaque handle per local actor incarnation and
uses the backend-neutral `ActorLifecycle<T>` contract. Mailbox admission is
bounded FIFO and requires the exact actor id/incarnation pair; stale references
are reported separately from full and closed mailboxes.

Integer payloads remain scalar ABI values. Managed payloads are retained as
precise runtime roots before admission and released or transferred exactly once
on rejection, receive, or actor exit. Native operations expose explicit create,
activate, send, receive, begin-exit, complete-exit, and release symbols.

No symbolic actor lookup, shared mutable actor state, class hierarchy, raw
managed pointer, suspension, scheduler policy, reply protocol, or supervision
is introduced by this slice.
