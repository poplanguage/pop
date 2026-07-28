# ADR 0121: Portable Base32 Codec

- Status: accepted
- Date: 2026-07-28
- Depends on: ADRs 0032, 0051, 0097, 0110, 0117, and 0118
- Supersedes: none

## Decision

`Pop.Bytes` appends:

```luau
public function Bytes.base32Encode(value: Bytes.View): String
public function Bytes.base32Decode(value: String): Bytes?
```

The codec uses the RFC 4648 uppercase `A` through `Z`, `2` through `7`
alphabet, emits required `=` padding, and produces no line breaks. Empty input
maps to empty output. Decode accepts only complete canonical eight-character
groups. It rejects lowercase and extended alphabets, whitespace, separators,
missing, excess, or interior padding, trailing data after padding, impossible
padding counts, and nonzero unused bits. Malformed input returns `nil`; valid
output owns independent storage.

Both operations are O(n) ordinary Pop source over typed views, linear String
iteration, reusable buffers, and checked UTF-8 finishing. They add no compiler
name recognition, intrinsic, native adapter, ambient behavior, or dynamic
fallback. Streaming, base32hex, human-oriented, and caller-buffer variants
remain separate contracts.

## Required conformance

- RFC 4648 empty and `f` through `foobar` vectors;
- alphabet boundaries, zero bytes, and arbitrary binary round trips;
- lowercase, extended alphabet, every invalid length, character, padding
  position/count, trailing datum, and nonzero unused-bit form returns `nil`;
- interpreter and real LLVM execution agree;
- checked public docs and reference metadata expose exactly two identities; and
- architecture tests reject compiler/runtime/backend duplicates.
