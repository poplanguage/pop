# ADR 0120: Portable Base64 Codec

- Status: accepted
- Date: 2026-07-28
- Depends on: ADRs 0032, 0051, 0097, 0110, 0117, and 0118
- Supersedes: none

## Decision

`Pop.Bytes` appends:

```luau
public function Bytes.base64Encode(value: Bytes.View): String
public function Bytes.base64Decode(value: String): Bytes?
```

The codec uses the RFC 4648 standard alphabet, emits required `=` padding, and
produces no line breaks. Empty input maps to empty output. Decode accepts only
complete canonical quartets: no whitespace, separators, URL-safe alphabet,
missing/excess/interior padding, trailing data after padding, or nonzero unused
bits. Malformed input returns `nil`; valid output owns independent storage.

Both operations are O(n) ordinary Pop source over typed views, linear String
iteration, reusable buffers, and checked UTF-8 finishing. They add no compiler
name recognition, intrinsic, native adapter, ambient behavior, or dynamic
fallback. Streaming, URL-safe, MIME-wrapped, and caller-buffer variants remain
separate contracts.

## Required conformance

- RFC 4648 empty and `f` through `foobar` vectors;
- all alphabet boundaries, zero bytes, and arbitrary binary round trips;
- uppercase/lowercase alphabet distinction;
- every invalid length, character, padding position/count, trailing datum, and
  nonzero unused-bit form returns `nil`;
- interpreter and real LLVM execution agree;
- checked public docs and reference metadata expose exactly two identities; and
- architecture tests reject compiler/runtime/backend duplicates.
