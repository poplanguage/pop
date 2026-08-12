# ADR 0149: Compiler-Proven Actor Message Safety

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0068 and 0148
- Supersedes: the unspecified first executable actor-message type proof

## Context

Actor isolation depends on copying a complete statically known message graph
into actor ownership. A marker interface, user-defined conversion, runtime
reflection test, or permissive generic fallback could admit mutable aliases,
borrowed values, executable state, or native resources. The compiler therefore
needs one closed recursive proof before actor send/capture lowering exists.

## Decision

`Pop.Standard` reserves three non-prelude nominal identities:

- `Actor.Ref<TMessage>` is an incarnation-scoped local actor reference;
- `Actor.Inbox<TMessage>` is the non-escapable actor-owned receive capability;
- `Actor.Reply<T>` is a single-use reply capability.

The compiler's first actor-message-safe graph contains:

- primitives and enums;
- fixed tuples and structural optional/union composition;
- immutable records;
- tagged unions whose every case payload is safe; and
- `Actor.Ref<T>` and `Actor.Reply<T>` when their generic payload is safe.

Recursive nominal graphs use a cycle-aware closed proof: revisiting an active
node is provisionally safe, but any reachable rejected node rejects the whole
graph. Unresolved or unknown types fail closed.

The proof rejects mutable arrays/tables and collection builtins, functions and
closures, classes, interfaces, attributes, arbitrary builtin/opaque values,
error placeholders, generic type parameters before concrete substitution, and
`Actor.Inbox<T>`. Consequently Tasks, Channels, borrowed views, buffers, FFI
values, resources, and native handles do not cross the actor boundary through a
generic fallback.

`Actor.Ref<T>` and `Actor.Reply<T>` being message-safe does not define their
runtime representation or public operations. `Actor.Inbox<T>` is compiler-known
only so actor-entry typing and escape checks can remain exact; it is never a
message.

## Required conformance

Tests prove nested record/tagged-union/tuple acceptance, exact actor reference
and reply recursion, and rejection of inboxes, mutable arrays/tables, callables,
and unresolved type parameters. Architecture regression tests require the
closed compiler query and forbid dynamic erasure or a source marker protocol.

## Consequences

Actor lowering can request one deterministic proof result and exact rejected
type identity before constructing copy maps. Capture paths, diagnostics,
copy-map generation, native isolated storage, and public operations remain
separate follow-on layers.
