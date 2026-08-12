# ADR 0132: Bounded Media Type Values

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0030, 0031, 0032, 0110, 0123, and 0129
- Supersedes: the planned-only first `Mime` catalog slice

## Context

HTTP, email, codecs, resources, and media packages need one portable owned media
type rather than repeated String conventions. A universal parameter table would
lose declaration order and invite dynamic lookup semantics. Automatically
sniffing content would mix a pure value parser with format-specific trust policy
and attacker-controlled byte heuristics. A complete extension registry also
requires generated versioned data that does not yet have an accepted update
contract.

The accepted ordinary-public-record path can carry a closed media-type value and
its ordered parameter records between Bubbles. Existing Text, Unicode, Bytes,
and List foundations are sufficient for a deterministic pure Pop parser.

## Decision

`Pop.Mime.Parameter` is an ordinary public record containing canonical lowercase
ASCII `name` and one decoded owned `value`. `Pop.Mime.Value` contains canonical
lowercase ASCII `mediaType`, canonical lowercase ASCII `subtype`, and a
declaration-ordered `List<Mime.Parameter>`.

`parse(String) -> Mime.Value?` consumes the complete input. Input is limited to
1,024 UTF-8 bytes and at most 32 parameters. Type, subtype, and names use a
nonempty ASCII token grammar: letters, digits, and
``!#$%&'*+-.^_`|~``. Optional whitespace is only ASCII space or tab around
semicolon-delimited parameters and their equals sign. Parameter values are
either nonempty tokens or quoted text containing tab and printable ASCII;
backslash quotes the following printable ASCII character. Empty quoted values
are valid. Empty sections, invalid tokens/quotes, duplicate
ASCII-case-insensitive names, non-ASCII grammar characters, and excess bounds
return absence.

The first public operations are:

- `format(Mime.Value) -> String`, which emits lowercase type/subtype, preserves
  parameter order, and minimally quotes/escapes values;
- `parameter(Mime.Value, String) -> String?`, which performs a bounded
  ASCII-insensitive name lookup and returns the decoded owned value; and
- `matches(Mime.Value, String) -> Boolean`, which ignores parameters and accepts
  only an exact `type/subtype`, `type/*`, or `*/*` identity pattern.

Parsed values are canonical. Manual public-record initializers are responsible
for the same token, uniqueness, and bound invariants; operations remain
deterministic for every statically typed record value. Structured suffix policy,
quality negotiation, extension lookup, generated registry data, and content
sniffing are outside this slice. Sniffing remains explicit and belongs to the
relevant format/media Package; it never silently overrides a declared type.

The implementation is ordinary Pop source. It uses no PLRI call, native mirror,
ambient registry, runtime reflection, dynamic lookup, or backend-specific
HIR/MIR operation.

## Consequences

- Portable libraries share one bounded canonical parser and formatter.
- Parameters retain deterministic declaration order and remain typed records,
  not a runtime String table.
- Exact and wildcard identity checks are available without importing HTTP
  negotiation policy.
- Registry and sniffing work cannot silently introduce generated data,
  filesystem access, or attacker-controlled detection into the pure core.

## Required conformance

- mixed-case token identities and names canonicalize while parameter values
  preserve decoded case;
- token and quoted parameter values parse/format, including escaped quote and
  backslash, and empty quoted values;
- malformed identity/parameter separators, invalid/non-ASCII tokens, duplicate
  names, broken escapes, oversized text, and excess parameters return absence;
- lookup is ASCII-insensitive and wildcard matching accepts only exact,
  `type/*`, and `*/*` patterns while ignoring parameters;
- documentation and the frozen standard API baseline include both records and
  four functions;
- the same source reaches verified HIR/MIR and executes on the MIR interpreter
  and LLVM backend; and
- no dynamic table, reflection, ambient registry, implicit sniffing, native
  duplicate, or backend-specific IR is added.

## Documents/components affected

Core library catalog, essential-library projection, closed decisions, standard
implementation plan, API baseline, ordinary `Pop.Standard` source,
documentation checks, and interpreter/LLVM conformance.
