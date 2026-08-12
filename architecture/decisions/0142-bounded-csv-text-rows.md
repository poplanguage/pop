# ADR 0142: Bounded CSV Text Rows

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0030, 0031, 0032, 0110, and 0118
- Supersedes: the planned-only first `Csv` text-row slice

## Context

CSV interchange needs a strict portable baseline before typed schema adapters,
streaming I/O, or spreadsheet policy. An unbounded permissive parser invites
memory exhaustion and silently accepts incompatible quoting/newline dialects.

## Decision

The first `Pop.Csv` API is:

- `parse(String) -> List<List<String>>?`; and
- `format(List<List<String>>) -> String?`.

The dialect is fixed: comma delimiter, double-quote quoting, doubled quotes
inside quoted fields, and CRLF output. Input accepts LF or CRLF record endings,
preserves record endings inside quoted fields, rejects bare CR, quotes inside
unquoted fields, and any characters after a closing quote except delimiter or
record ending.

Parsing and formatting are bounded to 1,048,576 input/output UTF-8 bytes, 4,096
rows, 4,096 fields per row, and 65,536 bytes per field. A final record ending
does not create an extra empty row. Empty input is one row containing one empty
field.

Formula-injection policy is not silently applied; callers choose it when
exporting to spreadsheet software. Configurable dialects, typed record/schema
mapping, incremental events, reusable row buffers, and `Io` streaming remain
later contracts.

## Required conformance

- empty, simple, quoted, escaped-quote, comma, CRLF/LF, embedded-newline, and
  trailing-record-ending cases round-trip;
- malformed quoting, bare CR, and every size/count boundary reject;
- checked documentation and the API baseline contain both functions;
- MIR interpreter and LLVM execute the same ordinary Pop implementation; and
- no dynamic row, reflection, spreadsheet mutation, I/O, native duplicate, or
  backend-specific IR is introduced.

## Consequences

The standard library gains deterministic typed text rows while leaving schema,
streaming, and application-specific safety policy explicit.
