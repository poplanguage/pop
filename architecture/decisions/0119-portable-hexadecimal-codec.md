# ADR 0119: Portable Hexadecimal Codec

- Status: accepted
- Date: 2026-07-28
- Depends on: ADRs 0032, 0051, 0097, 0110, 0113, 0117, and 0118
- Supersedes: none

## Context

Binary identifiers, digests, protocol fields, diagnostics, and user codecs need
a canonical hexadecimal boundary. Reimplementing it in each Package duplicates
case, malformed-input, odd-length, and allocation policy. Hexadecimal is a
portable byte algorithm; it does not require cryptography, locale behavior, a
native adapter, or compiler name recognition.

## Decision

`Pop.Bytes` appends two non-prelude functions:

```luau
public function Bytes.hexEncode(value: Bytes.View): String
public function Bytes.hexDecode(value: String): Bytes?
```

`hexEncode` emits exactly two lowercase ASCII digits per input byte, using
`0` through `9` and `a` through `f`. Empty input produces the empty String.

`hexDecode` accepts ASCII digits case-insensitively, so `A` through `F` and
`a` through `f` have the same values. Empty input produces empty Bytes. Any
non-ASCII digit, whitespace, separator, prefix, sign, or odd number of digits
returns `nil`. Decoding consumes the complete String and never ignores a byte.

The returned String and Bytes own independent immutable storage. Encoding and
successful decoding are O(n), allocate one reusable buffer plus one immutable
result, and do not perform ambient I/O, scheduling, suspension, interface
dispatch, locale conversion, normalization, or user callbacks. Invalid decode
may allocate the temporary buffer but never returns a partial result.

## Implementation and portability

Both functions are ordinary documented Pop source in `bytes.pop`. They use
typed integer arithmetic, `Bytes.View` inspection, linear String iteration,
`Unicode.codePoint`, ADR 0117's reusable buffer, and ADR 0118's checked UTF-8
finish. HIR and MIR contain only existing typed operations; the compiler,
runtime, interpreter, LLVM backend, and experimental C backend do not recognize
the source names.

The Standard API baseline appends both exact callable identities as
`implemented`. They remain outside the prelude and resolve in dependent Bubbles
through ordinary verified public reference metadata.

## Consequences

- User libraries gain one deterministic lowercase hexadecimal spelling.
- Decoders accept conventional uppercase input without producing mixed-case
  output.
- Malformed text remains explicit optional flow.
- Base32, base64, streaming codecs, destination-buffer forms, and bitwise byte
  algorithms remain separate contracts.

## Alternatives considered

### Return a typed error union

Rejected for this compact all-or-absence codec because there are only malformed
and valid outcomes and no limit/options surface. Rich streaming and bounded
decoders may require typed error detail later.

### Accept separators or a `0x` prefix

Rejected because context-dependent ignored syntax prevents one canonical
protocol decoder. Callers can parse presentation syntax before decoding.

### Use a native codec

Rejected because ordinary typed Pop expresses the linear algorithm and keeps
interpreter, LLVM, and future VM behavior governed by canonical MIR.

## Required conformance tests

- encoding covers empty input, zero, leading zeroes, and bytes producing every
  decimal and alphabetic digit;
- decoding covers lowercase, uppercase, mixed case, empty input, and exact
  round trips;
- odd length, whitespace, prefixes, separators, non-ASCII, and non-hex ASCII
  return `nil` without a partial result;
- source documentation records case, malformed input, complexity, allocation,
  and independence;
- public reference metadata exposes exactly the two appended identities;
- interpreter and real LLVM execution agree; and
- architecture tests reject a Rust duplicate, intrinsic, native adapter, or
  compiler/backend source-name recognition.

## Documents/components affected

Bytes source, Standard API baseline, core catalog, examples, implementation
inventory, architecture tests, roadmap, interpreter tests, and LLVM tests.
