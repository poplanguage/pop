# ADR 0184: Ordinary Public Enum Reference Metadata

- Status: Accepted
- Date: 2026-08-13
- Depends on: ADRs 0036, 0049, 0055, 0073, 0110, and 0129
- Supersedes: the primitive-and-record-only reference-metadata boundary for
  payload-free public enums

## Context

ADR 0049 defines closed nominal payload-free enums, but dependent Bubbles
cannot yet consume an ordinary public enum through `reference.metadata`.
Modern independently versioned Packages such as `Pop.Http` require exact enum
parameters and record fields without loading producer source, erasing values to
integers, or introducing runtime lookup.

## Decision

Reference metadata schema version 1 includes every non-generic `public` enum
from the producer Bubble. Each projection carries the exact producer
`SymbolIdentity`, Module, namespace, source name, declaration-ordered case
names, zero-based discriminants, and source span.

Consumers reconstruct one isolated nominal enum and its exact cases. Function
parameters/results and ordinary public-record fields may reference that enum by
producer identity. HIR and MIR retain the producer identity; runtime values
remain ADR 0049 nominal scalar enum values and never become integers, strings,
tagged unions, reflection entries, or dynamic lookups.

Metadata decoding rejects wrong-Bubble identities, duplicate enum identities,
duplicate or empty case names, noncontiguous or reordered discriminants,
dangling enum references, and namespace/name collisions. `internal` and
`private` enums never enter public metadata.

## Required proof

- public enums emit in identity order and survive canonical metadata round trip;
- internal and private enums do not emit;
- a dependent Bubble names cases, compares values, passes and returns the enum,
  and constructs/projects a public record containing that enum;
- exact producer identities survive HIR and MIR round trips;
- malformed identities, cases, discriminants, and dangling references fail
  before consumer HIR; and
- no source loading, reflection registry, string dispatch, integer erasure, or
  backend-specific HIR/MIR operation is introduced.

## Documents and components affected

Reference metadata emission, validation and loading; enum resolution and type
checking; HIR/MIR nominal references; `.poplib` artifacts; `Pop.Http`; tests;
and the library/loading architecture.
