# ADR 0138: Bounded Gregorian Date Values

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0030, 0031, 0032, 0110, and 0129
- Supersedes: the planned-only first `Time.Date` slice

## Context

Configuration, protocols, scheduling inputs, and later wall-clock projections
need a portable civil date independent of a clock, locale, time zone, or
platform calendar service. Unbounded years complicate interchange and permit
integer-boundary mistakes in ordinary arithmetic. A timestamp would also
incorrectly attach an offset and time of day to a civil date.

An ordinary record can carry a small immutable Gregorian value across Bubbles
without native layout or runtime support.

## Decision

`Pop.Time.Date` is an ordinary public record with `year`, `month`, and `day`
fields. Canonically constructed dates use the proleptic Gregorian calendar and
the closed interchange range `0001-01-01` through `9999-12-31`.

The first API is:

- `date(Int, Int, Int) -> Date?`, rejecting a year outside 1 through 9,999, a
  month outside 1 through 12, or a day outside that month;
- `isLeapYear(Int) -> Boolean`, applying the Gregorian divisible-by-four,
  century, and divisible-by-four-hundred rule to every `Int`;
- `daysInMonth(Int, Int) -> Int?`, rejecting an out-of-range interchange year or
  month; and
- `compareDates(Date, Date) -> Int`, returning negative one, zero, or one by
  year, month, then day without subtraction.

Manual record initializers remain responsible for the field invariant.
Comparison stays deterministic for every statically typed record. Construction
is the canonical validation path.

Date arithmetic, ordinal/day-of-week projections, parsing, formatting, times of
day, offsets, zones, calendars other than Gregorian, wall-clock acquisition,
and locale behavior remain later focused contracts. Leap seconds do not affect
a civil date.

The implementation is ordinary Pop source with no ambient clock, PLRI call,
runtime reflection, dynamic lookup, or backend-specific HIR/MIR operation.

## Consequences

- Portable APIs gain an exact civil date without acquiring time authority.
- The frozen range matches common four-digit interchange while keeping every
  calculation far from `Int` overflow.
- Later date-time and scheduling contracts reuse one validated date identity.

## Required conformance

- minimum/maximum dates, ordinary month boundaries, February, and invalid fields
  are covered;
- leap-year tests include ordinary, century, and four-hundred-year cases;
- month length rejects invalid interchange years and months;
- comparison covers each field, equality, and manually initialized extremes
  without subtraction;
- checked documentation and the frozen API baseline include the record and four
  functions;
- the same ordinary source reaches verified HIR/MIR and executes on the MIR
  interpreter and LLVM backend; and
- no clock, zone, locale, dynamic value, native duplicate, or backend-specific
  IR is added.

## Documents/components affected

Core catalog, closed decisions, standard implementation plan, API baseline,
ordinary `Pop.Standard` source, documentation checks, and interpreter/LLVM
conformance.
