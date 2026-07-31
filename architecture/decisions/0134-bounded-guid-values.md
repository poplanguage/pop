# ADR 0134: Bounded GUID Values

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0030, 0031, 0032, 0058, 0110, 0117, and 0129
- Supersedes: the planned-only `Guid` catalog row and ADR 0058's dormant
  bootstrap-only `Guid` nominal prelude binding

## Context

Identifiers need one portable 128-bit value with exact text and byte
interchange. The bootstrap prelude reserved a nominal `Guid` spelling, but
provided no construction, representation, operations, source declaration, or
reference metadata. Stabilizing that placeholder would require compiler and
backend name recognition or a second hidden value model.

Ordinary public records now cross Bubble boundaries. Four fixed-width words
preserve all 128 bits without dynamic storage, runtime reflection, or a special
HIR/MIR operation. Generation also needs an explicit randomness policy rather
than ambient entropy or the deterministic `Random.State` being mislabeled as
cryptographic randomness.

## Decision

The dormant prelude placeholder is removed. The public value is
`Pop.Guid.Value`, an ordinary record containing `firstWord`, `secondWord`,
`thirdWord`, and `fourthWord` in unsigned big-endian word order. Every bit
pattern is a valid GUID value. Code names the family explicitly with
`using Pop.Guid`; no duplicate root `Guid` type or compatibility alias remains.

The first operations are:

- `parse(String) -> Guid.Value?`, accepting exactly the 36-character
  `8-4-4-4-12` hexadecimal form, case-insensitively and without braces, URN
  prefixes, whitespace, or aliases;
- `format(Guid.Value) -> String`, returning canonical lowercase
  `8-4-4-4-12` text;
- `fromBytes(Bytes) -> Guid.Value?`, accepting exactly 16 owned bytes in network
  byte order;
- `toBytes(Guid.Value) -> Bytes`, returning 16 independently owned bytes in the
  same order;
- `isNil(Guid.Value) -> Boolean`, testing the all-zero value;
- `isVersion4(Guid.Value) -> Boolean`, testing the version-four nibble; and
- `newVersion4(Bytes) -> Guid.Value?`, which accepts exactly 16 caller-supplied
  owned random bytes, then sets the version nibble to four and the RFC variant
  bits to binary `10`.

The caller owns entropy acquisition, quality, blocking, failure, and security
policy before calling `newVersion4`. The function neither uses ambient entropy
nor claims that arbitrary bytes are cryptographically secure. Deterministic
bytes are suitable for tests; production callers obtain bytes from a
cryptographically secure capability under the later `Crypto` contract.

The ordinary record allocation and returned text/byte allocations are stated
honestly. A future proof-directed scalar replacement may remove record storage
without changing source identity. The API does not promise a special 128-bit
primitive, unboxed ABI, native layout, or zero allocation.

The first slice exposes only named nil and version-four inspection. It does not
return a version number or String that callers can dispatch dynamically.
General named version cases remain deferred until public enum identity and case
metadata can cross Bubble boundaries under an accepted extension to ADR 0129;
this slice does not pretend a source-only enum is a usable public contract.
Timestamp/name/hash-specific constructors, UUID namespace constants, monotonic
version-seven state, alternate textual forms, ordering, hashing protocols, and
database/native ABI layouts remain later focused contracts.

The implementation is ordinary Pop source with no PLRI call, native mirror,
ambient generator, runtime reflection, dynamic lookup, or backend-specific
HIR/MIR operation.

## Consequences

- GUID values have one explicit namespace-owned type rather than an unusable
  prelude placeholder plus a second implementation type.
- Canonical text and exact network bytes round-trip every 128-bit pattern.
- Version-four generation consumes explicit caller-owned randomness and is
  testable without blessing deterministic pseudo-randomness as secure entropy.
- The value crosses Bubble boundaries through ADR 0129 and uses the same MIR
  record semantics as other portable core values.

## Required conformance

- lowercase and uppercase canonical text parse, canonical lowercase text
  formats, and wrong length, separators, digits, prefixes, braces, or
  whitespace return absence;
- all-zero, boundary, and all-one byte patterns round-trip exactly through four
  words and 16 network-order bytes;
- named nil and version-four predicates inspect exact bits without a general
  runtime number/String dispatch API;
- version-four generation validates exact input length, preserves all
  non-version/variant random bits, and sets version four plus RFC variant `10`;
- the frozen prelude and API baselines remove the dormant root `Guid` type and
  include `Guid.Value` and the seven functions;
- checked documentation includes one compiled explicit-random-bytes example;
- the same source reaches verified HIR/MIR and executes on the MIR interpreter
  and LLVM backend; and
- no implicit entropy, deterministic-security claim, runtime name resolution,
  dynamic value, native duplicate, or backend-specific IR is added.

## Documents/components affected

Prelude and API baselines, core library catalog, essential-library projection,
closed decisions, standard implementation plan, ordinary `Pop.Standard`
source, documentation checks, and interpreter/LLVM conformance.
