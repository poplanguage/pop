# ADR 0122: Portable Bytes Bitwise Transforms

- Status: accepted
- Date: 2026-07-28
- Depends on: ADRs 0032, 0051, 0097, 0110, and 0117
- Supersedes: none

## Decision

`Pop.Bytes` appends:

```luau
public function Bytes.bitwiseAnd(
    left: Bytes.View,
    right: Bytes.View,
): Bytes?
public function Bytes.bitwiseOr(
    left: Bytes.View,
    right: Bytes.View,
): Bytes?
public function Bytes.bitwiseXor(
    left: Bytes.View,
    right: Bytes.View,
): Bytes?
public function Bytes.bitwiseNot(value: Bytes.View): Bytes
```

The binary transforms combine corresponding bits in equal-length views.
Different lengths return `nil`; neither side is truncated, extended, cycled,
or mutated. `bitwiseNot` flips all eight bits in every byte. Every successful
result owns independent immutable storage. Empty equal views and empty
`bitwiseNot` input produce empty Bytes.

All four operations are O(n) ordinary Pop source over typed immutable views and
one reusable output buffer. They add no source operator, compiler name
recognition, intrinsic, native adapter, ambient behavior, or dynamic fallback.
The absence of accepted source-level bitwise numeric operators does not change
the public byte semantics; a later operator contract may optimize the same
ordinary typed algorithms without changing these APIs.

The functions are ordinary data transforms, not constant-time cryptographic
comparisons or secret-key operations. Security-sensitive algorithms remain in
`Crypto` with separately proved timing and memory-clearing contracts.
Whole-sequence shifts and rotations remain outside this decision because their
bit order, width, and fill policy require a separate explicit contract.

This decision completes the phase-1 portable `Bytes` family defined by ADRs
0097, 0113, 0117, 0118, 0119, 0120, 0121, and this decision. The `Bytes` public
root and essential-family projection advance to `implemented`. ADR 0113's
twelve inspection/endian-read identities advance from `prototype` to
`implemented`; their signatures and behavior do not change.

## Required conformance

- all zero, all one, alternating, and mixed byte patterns;
- empty, one-byte, and multi-byte inputs;
- binary length mismatch returns `nil` without partial output;
- inputs remain unchanged and every output owns independent storage;
- interpreter and real LLVM execution agree;
- checked public docs and reference metadata expose exactly four identities;
  and
- the public-root, essential-family, and complete Bytes API statuses all read
  `implemented`; and
- architecture tests reject compiler/runtime/backend duplicates.
