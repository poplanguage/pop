# ADR 0123: Essential Text Algorithms and Integer Parsing

- Status: accepted
- Date: 2026-07-28
- Depends on: ADRs 0032, 0051, 0097, 0114, 0116, 0117, and 0118
- Supersedes: none

## Decision

`Pop.Unicode` appends:

```luau
public function Unicode.isWhitespace(value: Rune): Boolean
```

It implements the Unicode 17.0.0 `White_Space` property exactly, independent
of locale.

`Pop.Text` appends:

```luau
public function Text.trimStart(value: String): String
public function Text.trimEnd(value: String): String
public function Text.trim(value: String): String
public function Text.replace(
    value: String,
    target: String,
    replacement: String,
): String
public function Text.split(value: String, separator: String): List<String>
public function Text.join<TSource: Iterable<String>>(
    source: TSource,
    separator: String,
): String
public function Text.parseInt(value: String): Int?
```

Trim operates on Unicode scalar values and removes exactly the Unicode
`White_Space` property. Replace and split match exact UTF-8 text
left-to-right without overlap. An empty replacement target returns the input
unchanged. An empty split separator returns a one-item list containing the
input. Split preserves leading, interior, and trailing empty fields. Join
preserves source order and emits separators only between items.

`parseInt` accepts one or more ASCII decimal digits with one optional leading
`+` or `-`. It accepts no whitespace, separators, prefixes, non-ASCII digits,
or trailing text. Overflow, underflow, and malformed input return `nil`; the
complete `Int` range, including its minimum, is accepted.

The algorithms are ordinary typed Pop. Trim and parse are O(n) and allocate
only their returned owned value where needed. Join is O(n) in total UTF-8
bytes. Replace and split use exact matching in O(n * m) worst-case time for
input length n and pattern length m. All construction uses reusable buffers;
the algorithms do not repeatedly concatenate a growing String. No
compiler/runtime source-name recognition, ambient locale, dynamic fallback, or
native duplicate is introduced.

Grapheme/word/line segmentation, normalization, full case mapping, formatting,
floating-point parsing, templates, diffs, and tokenization remain later Text
and Unicode contracts. This decision advances both families but does not mark
either complete.

## Required conformance

- every Unicode `White_Space` range and representative neighboring scalar;
- empty, all-whitespace, leading, trailing, and mixed Unicode trim cases;
- replacement and split cover empty fields, multibyte text, non-overlap, no
  match, and empty target/separator policy;
- join covers empty, one, and multiple items with multibyte separators;
- parse covers signs, zero, both `Int` limits, overflow/underflow, malformed
  signs, whitespace, non-ASCII digits, and trailing text;
- interpreter and real LLVM execution agree;
- checked public docs and reference metadata expose exactly eight identities;
  and
- architecture tests reject compiler/runtime/backend duplicates.
