# ADR 0146: Native Bounded Channel ABI

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0068, 0078, 0103, and 0145
- Supersedes: the planned-only native bounded-channel adapter

## Context

ADR 0145 fixes portable channel admission and closure, but native code still
needs versioned physical operations. Queued managed payloads cannot be stored
as untracked raw tokens because a collection may run while their sender is no
longer a root.

## Decision

Native ABI 1.27 and production ABI 2.5 add these operations:

- create a bounded channel from an exact `u64` capacity and return a nonzero
  opaque handle;
- retain/release one sender or receiver endpoint and close the sender
  direction, each with a one-byte success result;
- try-send one `u64` payload plus the compiler-proven scalar/managed
  representation bit and return closed `Failure`, `Sent`, `Full`, or `Closed`;
  and
- try-receive through one writable `u64` output and return closed `Failure`,
  `Item`, `Empty`, or `Closed`, writing only for `Item`.

The symbols are `pop_rt_channel_create`, directional
`pop_rt_channel_retain_*`/`pop_rt_channel_release_*`,
`pop_rt_channel_close`, `pop_rt_channel_try_send`, and
`pop_rt_channel_try_receive`. PLRI names them as distinct `RuntimeOperation`
cases. No catch-all channel operation or runtime type tag exists.

Native storage instantiates ADR 0145's generic lifecycle. A managed payload is
converted to a precise strong root before admission. Full, closed, or failed
admission releases that provisional root; receive resolves and releases an
admitted root while the current managed mutator still owns the returned token.
Last-receiver release drains and releases every queued managed root. Scalar
payloads never enter the collector root set.

The operations are non-suspending. Scheduler wait queues, cancellation,
rendezvous pairing, public typed endpoints, MIR instructions, and selection
remain later coordinated layers and cannot redefine these statuses.

## Required conformance

ABI tests cover descriptor negotiation, unique symbols, every closed status,
FIFO/full/close behavior, endpoint retain/release, invalid representation
failure, managed-payload survival across collection, and root release after
receive or last-receiver close. Both stable and production facades expose the
same channel meanings; older descriptors remain immutable.

## Consequences

Native channel queues now preserve precise GC reachability without erased
payloads or backend-private closure semantics.
