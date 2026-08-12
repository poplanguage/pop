# ADR 0128: Materializing Sequence Order and Equality

- Status: accepted
- Date: 2026-07-28
- Depends on: ADRs 0032, 0051, 0053, 0064, 0075, and 0110
- Supersedes: none

## Decision

`Pop.Sequence` appends:

```luau
public function Sequence.reverse<T, TSource: Iterable<T>>(
    source: TSource,
): List<T>
public function Sequence.sort<T, TSource: Iterable<T>>(
    source: TSource,
    compare: function(left: T, right: T): Int,
): List<T>
public function Sequence.sortBy<T, TSource: Iterable<T>>(
    source: TSource,
    select: function(value: T): Int,
): List<T>
public function Sequence.containsBy<T, TSource: Iterable<T>>(
    source: TSource,
    value: T,
    equal: function(left: T, right: T): Boolean,
): Boolean
public function Sequence.equalsBy<
    T,
    TLeft: Iterable<T>,
    TRight: Iterable<T>,
>(
    left: TLeft,
    right: TRight,
    equal: function(left: T, right: T): Boolean,
): Boolean
```

`reverse` and `sort` eagerly consume the source into one new `List<T>`.
`reverse` swaps in place in O(n) time. `sort` is stable and uses an in-place
insertion sort over that list. Its first implementation is O(n²) worst-case,
O(n) best-case comparisons, and O(n) result storage. The comparator is a closed
pure function called only as needed in deterministic insertion order; a
positive result moves the left item after the right item, while zero preserves
source order.

`sortBy` applies its closed pure selector exactly once per source item, stores
the resulting `Int` keys in a parallel list, and performs the same stable
insertion order. It enables stable record sorting without widening the closed
callback contract or repeatedly projecting keys.

Insertion sort is chosen because the current generic `List<T>` supports exact
checked access and replacement without tuple-bearing auxiliary collections.
A later stable O(n log n) implementation may replace it without changing
observable order or comparator meaning.

`containsBy` is allocation-free and stops on the first match. It calls `equal`
with the source item first and requested value second.

`equalsBy` eagerly materializes both inputs because ordinary source cannot yet
exhaustively step two `Iteration<T>` values without fallback. It compares
length before invoking `equal`, then compares corresponding items from first
to last and stops on the first mismatch. A later no-materialization
implementation may replace it after exhaustive iteration matching is
available.

All five functions are ordinary typed Pop with no compiler/runtime name
recognition, dynamic fallback, hash requirement, or native duplicate.

## Required conformance

- empty, singleton, repeated, and already/reverse-ordered inputs;
- stable `sort`/`sortBy` order for equal keys, deterministic comparator order,
  and exactly one `sortBy` selector call per item;
- reverse returns independent storage and preserves the source;
- contains covers first/middle/last/no match and exact short-circuit counts;
- equality covers empty/equal/length mismatch/first/later mismatch and callback
  order/count;
- generic record and String elements execute;
- interpreter and real LLVM execution agree;
- checked public docs and reference metadata expose exactly five identities;
  and
- architecture tests reject an unstable comparison, native duplicate, hidden
  hash/equality requirement, and compiler name recognition.
