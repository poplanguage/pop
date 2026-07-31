# ADR 0140: Bounded Locale Tags

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0030, 0031, 0032, 0110, and 0129
- Supersedes: the planned-only first `Locale.Tag` slice

## Context

Localized resources and later formatting need a portable locale identity before
locale data, negotiation, or platform discovery exists. Ambient process locale
is nondeterministic and a raw string permits inconsistent casing and unchecked
subtag structure.

## Decision

`Pop.Locale.Tag` is an ordinary record containing canonical `language`, optional
`script`, and optional `region` strings. The first bounded parser accepts only:

- an ASCII language subtag of two through eight letters;
- an optional four-letter ASCII script subtag; and
- an optional region subtag of two ASCII letters or three ASCII digits.

Complete input is limited to 63 bytes. Language is lowercase, script is title
case, and alphabetic region is uppercase. Empty subtags, underscores, variants,
extensions, private-use subtags, non-ASCII text, and extra subtags are rejected.

The first API is:

- `parse(String) -> Tag?`;
- `format(Tag) -> String?`, revalidating manually initialized records; and
- `sameLanguage(Tag, Tag) -> Boolean`, comparing language case-insensitively
  without performing negotiation.

Full BCP 47 variants/extensions/private use, grandfathered mappings, likely
subtags, matching/negotiation, CLDR data, collation, messages, formatting, and
explicit platform locale discovery remain later contracts.

## Consequences

- Resources gain one deterministic owned locale key.
- Narrow initial syntax cannot silently claim full BCP 47 support.
- No ambient locale enters portable behavior.

## Required conformance

- language-only, language-region, language-script, and complete tags canonicalize;
- all field boundaries, malformed separators, non-ASCII, and unsupported
  subtags are rejected;
- formatting revalidates manual records and language comparison ignores ASCII
  case only;
- checked documentation and the frozen API baseline include one record and
  three functions;
- MIR interpreter and LLVM execute the same ordinary Pop source; and
- no locale discovery, CLDR lookup, dynamic value, native duplicate, or
  backend-specific IR is added.

## Documents/components affected

Core catalog, closed decisions, standard implementation plan, API baseline,
ordinary `Pop.Standard` source, documentation checks, and interpreter/LLVM
conformance.
