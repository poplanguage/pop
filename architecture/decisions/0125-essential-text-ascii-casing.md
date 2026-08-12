# ADR 0125: Essential Text ASCII Casing

- Status: accepted
- Date: 2026-07-28
- Depends on: ADRs 0032, 0051, 0114, 0117, 0118, and 0123
- Supersedes: none

## Decision

`Pop.Text` appends:

```luau
public function Text.toAsciiLower(value: String): String
public function Text.toAsciiUpper(value: String): String
public function Text.equalsAsciiIgnoreCase(
    left: String,
    right: String,
): Boolean
```

The conversions change only ASCII `A` through `Z` or `a` through `z`.
Every non-ASCII UTF-8 byte sequence is preserved exactly.
`equalsAsciiIgnoreCase` folds ASCII letters while comparing and treats all
non-ASCII text with exact case-sensitive equality. It returns false for
different UTF-8 byte lengths and does not normalize text.

All operations are O(n) ordinary typed Pop over checked UTF-8 and immutable
byte views. Conversions use one reusable output buffer and return owned String
results. Equality allocates encoded immutable inputs but no result storage and
stops at the first mismatch. No locale, compiler/runtime name recognition,
dynamic fallback, or native duplicate is introduced.

Full Unicode casing, case folding, normalization-aware comparison, and
locale-sensitive casing remain generated Unicode/Locale contracts.

## Required conformance

- empty, all-lower, all-upper, mixed ASCII, digits, punctuation, and embedded
  NUL;
- two-, three-, and four-byte non-ASCII text remains byte-exact;
- equality covers length mismatch, first/later mismatch, ASCII case pairs, and
  non-ASCII case sensitivity;
- interpreter and real LLVM execution agree;
- checked public docs and reference metadata expose exactly three identities;
  and
- architecture tests reject compiler/runtime/backend duplicates.
