# ADR 0114: Unicode Scalar Value and Text Access

- Status: accepted
- Date: 2026-07-26
- Depends on: ADRs 0003, 0030, 0032, 0040, 0051, 0058, 0078, 0097, 0103,
  0104, and 0110
- Supersedes: ADR 0103's exact production-facade version selection only

## Context

The accepted text model says that `String` is valid UTF-8 and that ordinary
text indexing and iteration operate in Unicode scalar values. The modern
library catalog also promises a `Rune` value. The bootstrap currently exposes
scalar-counted `Text.View` slicing but has no source type that can carry one
validated scalar and no operation that can inspect a scalar without
materializing a substring.

Using `UInt32` as the public scalar type would allow surrogate code points and
values above U+10FFFF to bypass validation. A record cannot hide its field or
enforce a construction invariant, and a class would add identity, allocation,
and dispatch to a small immutable value.

The complete Unicode family also requires generated property, normalization,
casing, and segmentation data. That data contract is separate from the
representation and scalar-access foundation needed to implement it.

## Decision

`Rune` is a source-visible primitive value type in the curated prelude. It is
not numeric and does not participate in arithmetic, bit operations, or numeric
conversion syntax. A `Rune` always contains exactly one Unicode scalar value:
U+0000 through U+D7FF or U+E000 through U+10FFFF. Its portable physical value
is an unsigned 32-bit scalar, but that representation is not a source-level
conversion or FFI contract.

`Rune` has value equality. There is no rune literal in this slice. Values enter
ordinary source only through validated text decoding or the following checked
constructor:

```luau
public function Unicode.fromCodePoint(value: UInt32): Rune?
public function Unicode.codePoint(value: Rune): UInt32
public function Text.get(text: String, index: Int): Rune?
public function Text.get(view: Text.View, index: Int): Rune?
```

`Unicode.fromCodePoint` returns absence for U+D800 through U+DFFF and for values
above U+10FFFF. `Unicode.codePoint` is total and returns the exact scalar code
point.

`Text.get` uses one-based Unicode-scalar indexing. It returns absence when the
index is less than one or greater than the scalar length. It never returns a
UTF-8 code unit, replacement value, or partial sequence. The `String` overload
creates only a compiler-proven ephemeral view; neither overload allocates,
copies, mutates, suspends, or crosses a native boundary in portable MIR.

The first ordinary `Pop.Unicode` source contribution adds allocation-free ASCII
helpers over `Rune`:

```luau
public function isAscii(value: Rune): Boolean
public function isAsciiLetter(value: Rune): Boolean
public function isAsciiDigit(value: Rune): Boolean
public function isAsciiAlphanumeric(value: Rune): Boolean
public function isAsciiWhitespace(value: Rune): Boolean
public function toAsciiLower(value: Rune): Rune
public function toAsciiUpper(value: Rune): Rune
```

ASCII whitespace is exactly horizontal tab, line feed, vertical tab, form feed,
carriage return, and space. Case conversion changes only `A` through `Z` or
`a` through `z`; every other rune is returned unchanged.

## Unicode data version

The complete Unicode family will generate immutable tables from Unicode
17.0.0, whose stable Unicode Character Database is documented by
[Unicode Standard Annex #44](https://www.unicode.org/reports/tr44/tr44-36.html).
Generated tables must record that exact version and deterministic source-file
hashes. A later Unicode-data update is a deliberate compatibility change with
normalization, casing, segmentation, width, and property conformance updates.

This scalar slice does not claim that property tables, normalization, full
casing, grapheme/word/line segmentation, or display width are implemented.

## IR and runtime contract

HIR and canonical MIR gain backend-neutral operations for:

- checked `UInt32` to `Rune`;
- exact `Rune` to `UInt32`; and
- optional scalar access from a checked `Text.View`.

The operations carry typed operands and results, not source names or native
symbols. MIR verification requires the exact `UInt32`, `Rune`, `Text.View`, and
optional result types. The MIR interpreter decodes already-valid UTF-8
directly. LLVM lowers `Rune` to `i32` and text access to one typed PLRI/native
view operation. A future VM consumes the same MIR contract.

The native operation advances the additive stable-token descriptor to ABI
1.23 and the production writable-root descriptor to ABI 2.1. Both descriptors
add the exact `pop_rt_text_view_get_rune` entry: managed string token, checked
byte offset, checked byte length, checked scalar length, and signed one-based
index plus a writable `u32` output location produce a one-byte presence status.
Success writes one validated scalar payload and returns `1`; failure or absence
returns `0` without changing the output location. This explicit status/output
shape avoids target-dependent small-aggregate C calling conventions.
ABI 1.11 through 1.22 and ABI 2.0 remain immutable descriptors. The default
facade supports ABI 1.11 through 1.23; the production facade supports ABI 2.0
and 2.1 and rejects ABI 1.

`Rune` is not a managed reference, GC root, native handle, reflective value, or
dynamic container. Aggregates containing runes use scalar storage and precise
maps that do not mark rune slots.

## Cost contract

- checked construction and code-point extraction are O(1), direct, and
  allocation-free;
- ASCII classification and conversion are O(1), direct, and allocation-free;
- `Text.get` is O(n) in UTF-8 bytes inspected before the requested scalar with
  O(1) storage in the first implementation;
- no operation performs locale-sensitive behavior;
- no operation schedules, blocks, suspends, or performs ambient I/O.

Implementations may cache scalar offsets or vectorize generated table lookup
only when they preserve the exact typed behavior and view lifetime contract.

## Consequences

- User libraries gain a statically valid scalar value instead of passing raw
  integers through text APIs.
- Unicode tables and text algorithms have one safe scalar representation on
  every backend.
- The prelude grows by one deliberate primitive binding.
- `Unicode` remains incomplete until its pinned generated data, normalization,
  full casing, segmentation, width, documentation, and conformance gates pass.
- `Text` remains incomplete until its broader search, split/join, replacement,
  parsing, formatting, construction, and iteration contracts pass.

## Alternatives considered

### Use `UInt32`

Rejected because it cannot prove the scalar invariant and makes every consumer
repeat or forget surrogate and maximum checks.

### Use a record

Rejected because the current record model cannot hide construction of an
invalid field value and would not provide the required nominal scalar identity.

### Use a class

Rejected because stable object identity, managed allocation, and lifecycle are
the wrong semantics and cost for one immutable scalar value.

### Return one-character `String`

Rejected because it allocates or aliases owned storage, obscures scalar
validation, and makes property-table indexing unnecessarily expensive.

## Required conformance tests

- the bootstrap schema contains exactly one canonical prelude `Rune` primitive
  and no `Char`, `Character`, or numeric alias;
- invalid surrogate and above-maximum code points return absence while boundary
  scalars round-trip exactly;
- `Text.get` covers ASCII, two-, three-, and four-byte UTF-8 scalars, sliced
  views, first/final indexes, zero, negative, and out-of-range indexes;
- rune equality works while arithmetic and unchecked numeric conversion fail
  statically;
- ASCII helpers cover every boundary and preserve non-ASCII runes;
- HIR/MIR verification rejects type drift for every new operation;
- MIR interpreter and LLVM execution agree;
- native ABI 1.23/2.1 identity, symbol shape, UTF-8 boundaries, and prior
  descriptor compatibility pass;
- GC/reference maps treat `Rune` as scalar;
- public checked documentation states indexing, absence, allocation, boundary,
  and complexity; and
- architecture tests reject dynamic fallback, Rust algorithm duplicates, or
  compiler/backend recognition of the ordinary ASCII helper names.

## Documents/components affected

Primitive/bootstrap schema, type checking, HIR, MIR, interpreter, LLVM, native
view adapter, Standard API baseline and source inventory, Unicode/Text catalog,
closed decisions, examples, documentation tests, architecture tests, and
roadmap.
