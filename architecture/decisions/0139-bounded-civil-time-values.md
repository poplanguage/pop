# ADR 0139: Bounded Civil Time Values

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0030, 0031, 0032, 0110, 0129, and 0138
- Supersedes: the planned-only first civil time, local date-time, and offset slice

## Context

Protocols, configuration, and later wall-clock projections need to distinguish
a time of day, a local date-time, a fixed UTC offset, and a date-time carrying
that offset. Treating them as one timestamp erases whether an instant, zone, or
calendar interpretation is available.

These are pure bounded values. They do not require clock authority, a time-zone
database, locale data, or runtime support.

## Decision

`Pop.Time.TimeOfDay` stores `hour`, `minute`, `second`, and `nanosecond`.
Canonical values use the closed 24-hour range: hour 0 through 23, minute and
second 0 through 59, and nanosecond 0 through 999,999,999. Leap-second spelling
is not accepted by this first value.

`Pop.Time.LocalDateTime` stores one `Date` and one `TimeOfDay`. It deliberately
has no offset, zone, or instant semantics.

`Pop.Time.UtcOffset` stores exact signed offset seconds. Canonical values range
from -64,800 through +64,800 seconds, inclusive.

`Pop.Time.OffsetDateTime` stores one `LocalDateTime` and one `UtcOffset`. It
carries enough information for later instant conversion but is not itself a
zone-aware value.

The first API is:

- `timeOfDay(Int, Int, Int, Int) -> TimeOfDay?`;
- `localDateTime(Date, TimeOfDay) -> LocalDateTime?`;
- `utcOffset(Int) -> UtcOffset?`;
- `offsetDateTime(LocalDateTime, UtcOffset) -> OffsetDateTime?`; and
- `isUtc(UtcOffset) -> Boolean`.

Each aggregate constructor revalidates manually initialized nested records.
Public record initializers remain responsible for their field invariants.

Parsing, formatting, arithmetic, ordering/instant conversion, offset component
projection, leap-second tables, named zones, daylight-saving transitions,
calendar systems, locale behavior, and wall-clock acquisition remain later
focused contracts.

The implementation is ordinary Pop source with no ambient clock, PLRI call,
runtime reflection, dynamic lookup, or backend-specific HIR/MIR operation.

## Consequences

- Public APIs express whether they carry a local civil value or a fixed offset.
- Named-zone identity cannot be confused with a fixed UTC offset.
- Later RFC/date formatting and wall-clock contracts reuse closed validated
  records.

## Required conformance

- minimum/maximum field values construct and each adjacent invalid value is
  rejected;
- local and offset aggregates reject manually invalid nested records;
- the UTC predicate is exact;
- checked documentation and the frozen API baseline include four records and
  five functions;
- the same ordinary source reaches verified HIR/MIR and executes on the MIR
  interpreter and LLVM backend; and
- no host clock, named zone, locale, dynamic value, native duplicate, or
  backend-specific IR is added.

## Documents/components affected

Core catalog, closed decisions, standard implementation plan, API baseline,
ordinary `Pop.Standard` source, documentation checks, and interpreter/LLVM
conformance.
