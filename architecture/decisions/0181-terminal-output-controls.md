# ADR 0181: Terminal Output Controls

- Status: Accepted
- Date: 2026-08-12
- Depends on: ADR 0032, ADR 0069

## Decision

The initial terminal output surface gains two direct, typed operations:

- `Terminal.writeError(value: String) -> Boolean` writes the validated string
  to the host standard-error stream and reports whether the write and flush
  succeeded;
- `Terminal.flush() -> Boolean` flushes the host standard-output stream.

Neither operation accepts a stream name, performs runtime lookup, changes
terminal modes, or exposes a universal stream object. Redirected hosts retain
ordinary byte-stream behavior. Both operations carry `AmbientIo` and are
available through the same native bridge and backend lowering as existing
terminal output.

## Required proof

The exact signatures must be present in the Standard bootstrap and API
baselines, with descriptor, MIR, LLVM, and native bridge coverage. Invalid
managed strings fail closed for `writeError`.
