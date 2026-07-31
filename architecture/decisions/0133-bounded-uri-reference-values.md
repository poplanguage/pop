# ADR 0133: Bounded URI Reference Values

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0030, 0031, 0032, 0110, 0123, and 0129
- Supersedes: the planned-only first `Uri` catalog slice

## Context

Network, resource, package, and format APIs need one portable URI-reference
value. Treating references as undifferentiated Strings repeats delimiter and
percent-escape parsing and loses the distinction between an absent and an empty
authority, query, or fragment. Performing DNS, HTTP, scheme dispatch, or IDNA
processing in that value would mix pure syntax with ambient capabilities and
policy.

The ordinary-public-record path can carry owned URI components between Bubbles.
The Text, Unicode, Bytes, and List foundations are sufficient for a bounded
deterministic implementation and lexical reference resolution.

## Decision

`Pop.Uri.Value` is an ordinary public record with lowercase `scheme`, optional
owned `authority`, exact percent-encoded `path`, optional exact `query`, and
optional exact `fragment`. Empty scheme denotes a relative reference. Optional
fields distinguish absence from a present empty component.

`parse(String) -> Uri.Value?` consumes one complete ASCII URI reference of at
most 4,096 bytes. It rejects spaces, controls, non-ASCII source characters,
malformed percent escapes, repeated fragment delimiters, and invalid schemes.
A scheme begins with an ASCII letter and continues with ASCII letters, digits,
plus, hyphen, or period. Its spelling canonicalizes to lowercase. Authority
starts only after `//` at the component start and otherwise remains opaque;
host, port, user-information, IPv6, IDNA, and scheme-specific validation belong
to typed later APIs. Other encoded component spelling is preserved exactly.

The first operations are:

- `format(Uri.Value) -> String`, preserving exact encoded components and
  absence/present-empty delimiters while lowercasing the scheme;
- `percentEncode(String) -> String?`, encoding UTF-8 bytes except ASCII
  unreserved letters, digits, hyphen, period, underscore, and tilde with
  uppercase escapes, and returning absence beyond 4,096 input bytes;
- `percentDecode(String) -> String?`, accepting at most 4,096 ASCII encoded
  characters and returning absence for malformed escapes or invalid resulting
  UTF-8; and
- `resolve(Uri.Value, Uri.Value) -> Uri.Value`, applying hierarchical
  reference resolution, query inheritance, authority replacement, and lexical
  dot-segment removal without decoding percent escapes.

Resolution preserves duplicate and empty non-dot path segments. Attempts to
walk above the available relative/root prefix are discarded. A reference
scheme replaces the base completely; a reference authority inherits only the
base scheme; an empty reference path inherits the base path and, when absent,
the base query. The reference fragment never inherits.

Raw Unicode/IRI and IDNA conversion, host/port decomposition, canonical
percent-escape normalization, query key/value policy, scheme handlers, DNS,
HTTP transport, trust decisions, and network access are outside this slice.
`Uri.Query` remains planned until its duplicate/order/encoding contract is
accepted.

The implementation is ordinary Pop source with no PLRI call, native mirror,
ambient scheme registry, runtime reflection, dynamic lookup, or backend-specific
HIR/MIR operation.

Exact optional managed fields remain precise GC references. Native lowering may
encode their private object slots as nullable handles, with zero representing
absence, provided SSA values preserve the typed optional shape and stack maps,
safe-point roots, loads, and stores all trace the present handle. This private
representation does not add an optional-URI or backend-specific HIR/MIR
operation.

## Consequences

- Portable code shares exact owned URI-reference components and deterministic
  lexical resolution.
- Empty query/fragment/authority delimiters round-trip instead of collapsing to
  absence.
- Percent coding is component-oriented and validates UTF-8 without smuggling
  transport or host policy into the value.
- Later URL/HTTP and IDNA APIs can layer typed policy over the opaque authority
  without changing this record identity.

## Required conformance

- absolute, authority-relative, path-relative, query-only, fragment-only, and
  empty references parse and format with exact optional-component presence;
- invalid schemes, whitespace/control/non-ASCII source, malformed percent
  escapes, repeated fragments, and oversized inputs return absence;
- UTF-8 percent encoding uses uppercase escapes, decoding validates UTF-8, and
  both bounds reject excess input;
- standard hierarchical base/reference cases cover scheme and authority
  replacement, path merging, dot segments, query inheritance, and fragment
  replacement while preserving empty segments;
- documentation and the frozen standard API baseline include the record and
  five functions;
- the same source reaches verified HIR/MIR and executes on the MIR interpreter
  and LLVM backend; and
- exact optional managed fields have precise object maps and safe-point roots
  in native execution; and
- no dynamic map, reflection, ambient scheme handler, network capability,
  native duplicate, or backend-specific IR is added.

## Documents/components affected

Core library catalog, essential-library projection, closed decisions, standard
implementation plan, API baseline, ordinary `Pop.Standard` source,
documentation checks, and interpreter/LLVM conformance.
