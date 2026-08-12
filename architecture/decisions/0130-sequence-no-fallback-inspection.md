# ADR 0130: Sequence No-Fallback Inspection

- Status: accepted
- Date: 2026-07-30
- Depends on: ADRs 0053, 0054, 0055, 0064, and 0073
- Supersedes: the deferred no-fallback inspection choice in ADR 0064

## Context

ADR 0064 deliberately exposed only fallback-taking `firstOr` and `lastOr`.
`T?` cannot distinguish an empty source from an item that is itself absent, and
ordinary source could not then exhaustively inspect the reserved
`Iteration<T>` union.

Reserved `Iteration<T>` now has exact exhaustive source matching, verified
typed HIR/MIR, portable generic-capsule reconstruction, and interpreter/LLVM
execution. Keeping basic no-fallback inspection unnamed would leave callers
either inventing sentinels or reimplementing traversal despite the accepted
closed presence carrier.

## Decision

`Pop.Sequence` adds exactly:

```luau
public function first<T, TSource: Iterable<T>>(
    source: TSource
): Iteration<T>

public function last<T, TSource: Iterable<T>>(
    source: TSource
): Iteration<T>
```

`first` consumes at most one item. It returns `Iteration.Item(value)` for that
item or `Iteration.End` for an empty source. `last` consumes the complete
source, retains only the latest item, and returns `Iteration.End` only when no
item was observed. Neither function rewinds or probes a source twice.

Both functions are ordinary generic `.pop` implementations. They allocate no
collection or adapter storage of their own and use O(1) space excluding
source-owned iterator state. `first` is O(1) in examined items and `last` is
O(n). Their result remains distinct from `T?`, `Result`, tuples, Boolean
sentinels, traps, and dynamic carriers.

The functions append implemented API identities to the versioned Standard
baseline. Portable reference metadata and specialization capsules preserve the
exact `Iteration<T>` result and private transitive helpers without adding
compiler, runtime, or backend name recognition.

An optional `Iteration.Item` payload keeps its inner presence bit distinct from
the outer `Item`/`End` case. Canonical MIR records tag, presence, and payload as
three verified logical object slots; native lowering reconstructs the exact
optional value without a sentinel, dynamic carrier, or hidden allocation.

## Consequences

- Optional source items remain distinguishable from an empty source.
- Callers can exhaustively handle the exact `Item` and `End` cases.
- Existing `firstOr` and `lastOr` remain direct conveniences and retain their
  signatures.
- Predicate search, positional inspection, reduction, minimum, and maximum
  no-fallback variants require later focused contracts; this ADR does not infer
  their names.

## Alternatives considered

### Return `T?`

Rejected because `Iterable<T?>` would collapse an absent item and an empty
source.

### Return `Result<T, Empty>`

Rejected because emptiness is ordinary iteration state and the reserved
`Iteration<T>` carrier already expresses the exact boundary.

### Add every no-fallback terminal at once

Rejected because predicate, indexing, reduction, and ordering families have
separate naming, callback-count, and empty-source contracts.

## Required conformance tests

- empty, single-item, and multi-item sources cover both functions;
- `first` proves at-most-one stepping and `last` proves complete ordered
  traversal;
- `Iterable<T?>` distinguishes `Iteration.Item(nil)` from `Iteration.End`;
- exhaustive source matching accepts exactly `Item` and `End`;
- cross-Bubble reference metadata and generic capsules retain the exact result;
- MIR interpreter, optimized MIR, and LLVM execution agree;
- checked documentation and the append-only API baseline agree; and
- architecture tests reject compiler/runtime/backend name recognition or a
  duplicate non-Pop implementation.

## Documents/components affected

Sequence source and checked documentation, Standard API baseline, library
catalog and examples, implementation roadmap, closed decisions, reference
metadata/capsule tests, MIR interpreter and LLVM tests, and architecture
conformance.
