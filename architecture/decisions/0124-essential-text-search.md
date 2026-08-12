# ADR 0124: Essential Text Search

- Status: accepted
- Date: 2026-07-28
- Depends on: ADRs 0032, 0051, 0097, 0114, 0116, 0118, and 0123
- Supersedes: none

## Decision

`Pop.Text` appends:

```luau
public function Text.startsWith(value: String, prefix: String): Boolean
public function Text.endsWith(value: String, suffix: String): Boolean
public function Text.contains(value: String, pattern: String): Boolean
public function Text.indexOf(
    value: String,
    pattern: String,
    start: Int,
): Int?
```

Matching is exact, case-sensitive Unicode text equality. `indexOf` returns the
one-based Unicode scalar index of the first match at or after `start`.
`start` may range from one through scalar length plus one. Values outside that
range return `nil`. The empty pattern matches at every valid boundary, so
`indexOf(value, "", start)` returns `start`; `contains` is always true for an
empty pattern. Empty prefixes and suffixes match every String.

The implementation uses ordinary typed Pop and exact UTF-8 matching. Valid
UTF-8 self-synchronization ensures a complete encoded pattern cannot begin in
the middle of a scalar. Search reports only decoded scalar boundaries, so the
public index never exposes a byte offset. The first implementation is O(n * m)
worst-case and may allocate encoded inputs plus split fields. A later
linear-time, non-materializing search optimization may replace the private
algorithm without changing results.

No locale, normalization, collation, compiler/runtime source-name recognition,
dynamic fallback, or native duplicate is introduced. Locale-aware or
normalization-insensitive search remains a separate `Locale`/Unicode contract.

## Required conformance

- empty value/pattern and every valid/invalid start boundary;
- ASCII and one-, two-, three-, and four-byte scalar positions;
- exact case sensitivity, prefix/suffix longer than input, and no match;
- repeated/overlapping candidates return the first permitted scalar index;
- interpreter and real LLVM execution agree;
- checked public docs and reference metadata expose exactly four identities;
  and
- architecture tests reject compiler/runtime/backend duplicates.
