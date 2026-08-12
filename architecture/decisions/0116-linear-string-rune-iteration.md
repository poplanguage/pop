# ADR 0116: Linear `String` Rune Iteration

- Status: accepted
- Date: 2026-07-28
- Depends on: ADRs 0053, 0054, 0056, 0078, 0097, 0103, 0104, 0110, and 0114
- Supersedes: ADR 0114's exact native-facade version selection only

## Context

The closed text model says that `String` is immutable valid UTF-8 and that its
default iteration unit is a Unicode scalar value. ADR 0114 supplies the exact
validated `Rune` item type, but generalized iteration does not yet accept a
`String`.

Library code could repeatedly call scalar-indexed `Text.get`. Because that
operation starts from the beginning of the selected view, a full traversal
would be O(n²) in UTF-8 bytes. Materializing an array or list of runes would
avoid repeated decoding but add storage and allocation that ordinary scanning
does not need.

`Text.View` cannot simply use the managed iterator protocol. ADR 0097
deliberately excludes iterator-yielded borrowed values until a later
container/iterator lifetime contract is accepted.

## Decision

Owned immutable `String` is one trusted compiler-known implementation of
exactly `Iterable<Rune>`. The direct source form is:

```luau
for rune in text do
    consume(rune)
end
```

Iteration yields each Unicode scalar value in source order exactly once. Empty
strings yield no items. Every item is a valid `Rune`; valid `String` storage
means iteration never substitutes U+FFFD, yields a UTF-8 code unit, or exposes
malformed input. Embedded U+0000 is an ordinary scalar.

Each `iterator()` acquisition creates one independent single-pass
`Iterator<Rune>` session. The source expression is evaluated once, acquisition
happens once, each step decodes at most one scalar, and exhaustion remains
stable. Because `String` is immutable, concurrent modification does not apply.
All generalized-loop control, cleanup, safe-point, and no-implicit-disposal
rules remain those of ADR 0053.

`String` satisfies an exact `TSource: Iterable<Rune>` generic bound and
participates in the same inference and portable specialization path as arrays,
lists, tables, ranges, and nominal implementations. It does not satisfy
`Iterable<T>` for any other `T`. No implicit interface value, runtime witness,
member lookup, conversion, or dynamic fallback is created.

`Text.View` remains non-iterable in this slice. Grapheme, word, line, byte, and
locale-sensitive iteration are separate explicitly named algorithms or later
accepted contracts.

## IR and runtime contract

Typed bodies and HIR retain `String` as one closed generalized-iteration source
with exact item type `Rune`. Generic specialization rewrites a proven
`Iterable<Rune>` source to that same closed source. Canonical MIR continues to
contain the reserved statically identified `iterator()` and `next()` calls,
`Iteration<Rune>` tests/projection, branches, and safe points. It contains no
UTF-8 decoder, native symbol, source name, byte pointer, or dynamic operation.

The MIR interpreter stores a byte offset in its private iterator session and
decodes one scalar from the already-valid UTF-8 value per step. LLVM selects
the closed native collection kind from the compiler-proven receiver type.

`IterationCollectionKind` adds the append-only `String = 4` discriminant.
Native acquisition validates the managed `String` token and records its exact
byte length. Each native step validates the token, decodes one scalar at the
current byte offset, writes its `u32` payload through the existing `u64`
iteration output slot, and advances by that scalar's UTF-8 width. No step
allocates, copies text, or scans earlier bytes. The one iterator-state object
uses the existing closed native layout.

This additive operation advances the stable facade to ABI 1.24 and the
production facade to ABI 2.2. ABI 1.11 through 1.23 and ABI 2.0 through 2.1
remain immutable. The default facade supports ABI 1.11 through 1.24; the
production facade supports ABI 2.0 through 2.2 and rejects ABI 1.

The experimental C backend rejects this runtime-dependent iteration explicitly.
A future conforming backend consumes the same verified MIR protocol calls.

## Cost contract

- acquisition evaluates the source once and allocates one small typed iterator
  session in the current native and interpreter implementations;
- a successful step is O(1) in the maximum four UTF-8 bytes of one scalar and
  allocates no storage;
- a complete traversal is O(n) in UTF-8 bytes with O(1) iterator state;
- iteration never materializes a rune collection or owned substring; and
- no operation schedules, blocks, suspends, performs ambient I/O, or invokes
  locale behavior.

Portable optimization may stack-promote or scalar-replace a nonescaping
session only when exact identity, roots, safe points, order, and traps remain
unchanged.

## Consequences

- User libraries can implement Unicode-aware scanners and algorithms in one
  linear pass without byte access or repeated scalar indexing.
- Ordinary `Sequence` operations can consume `String` as
  `Iterable<Rune>` through existing generic contracts.
- `Rune` remains nonnumeric and nonmanaged.
- Borrowed-view lifetime rules stay closed rather than being weakened for
  convenience.
- `Text` and `Unicode` remain incomplete until their remaining algorithms and
  generated-data gates pass.

## Alternatives considered

### Repeatedly call `Text.get`

Rejected because scalar-indexed access restarts decoding and makes a complete
scan O(n²).

### Materialize `Array<Rune>` or `List<Rune>`

Rejected because traversal does not require proportional storage or a copied
collection.

### Make `Text.View` iterable now

Rejected because a managed iterator carrying a borrowed range would bypass ADR
0097's accepted lender/provenance boundary.

### Iterate bytes

Rejected because bytes are available only through explicit byte APIs and the
closed default unit is a Unicode scalar.

## Required conformance tests

- direct typing covers empty, ASCII, two-, three-, and four-byte scalars,
  embedded U+0000, combining sequences, repeatable acquisition, and stable
  exhaustion;
- exact generic inference and specialization accept
  `TSource: Iterable<Rune>` while rejecting `Iterable<Int>` and other item
  types;
- `Text.View` iteration remains a static error;
- HIR verification rejects source/item drift, generic specialization selects
  the closed `String` source, and MIR contains only reserved protocol calls;
- MIR-interpreter and LLVM execution agree on scalar order and code points;
- native ABI 1.24/2.2 identity, append-only kind value, token validation,
  decoding boundaries, exhaustion, and prior descriptor compatibility pass;
- runtime allocation/root maps treat yielded `Rune` values as scalar and reuse
  the existing iterator-session layout;
- the experimental C backend fails closed; and
- architecture tests reject repeated-`Text.get` traversal, byte iteration,
  dynamic lookup, or a managed `Text.View` iterator.

## Documents/components affected

Closed decisions, type checking and generic inference, typed bodies, HIR,
canonical MIR lowering, MIR interpreter, LLVM, native ABI/runtime iteration,
experimental-C capability validation, standard-library catalog/examples,
conformance policy, roadmap, and performance evidence.
