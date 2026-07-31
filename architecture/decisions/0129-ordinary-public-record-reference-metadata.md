# ADR 0129: Ordinary Public Record Reference Metadata

- Status: accepted
- Date: 2026-07-28
- Depends on: ADRs 0017, 0036, 0054, 0055, 0073, and 0110
- Supersedes: ADR 0036's primitive-only first reference-metadata slice for
  non-generic ordinary public records

## Context

The modern core value families require immutable records. The compiler already
has a closed `ReferenceType.Record` identity and reconstructs trusted FFI layout
records, but ordinary public records are omitted unless they carry
`Ffi.C.Layout`. A dependent Bubble therefore cannot name, construct, inspect,
pass, or return the record values required by `Guid`, `Version`, `Uri`, and
`Mime`.

Using classes would add reference identity and mutable lifecycle to value APIs.
Loading producer source would collapse the Bubble boundary. Erasing records to
tuples or runtime maps would lose nominal identity and static fields.

## Decision

`reference.metadata` schema version 1 includes every non-generic `public`
record from the producer Bubble. Each projection carries:

- exact producer `SymbolIdentity`, Module, namespace, source name, and span;
- declaration-ordered field names and closed recursive `ReferenceType` values;
  and
- no producer-local `TypeId`, `FieldId`, source body, runtime reflection data,
  layout guess, or private declaration.

`internal` and `private` records never enter the projection. Every referenced
record identity must have exactly one sorted projection owned by the metadata
Bubble. Field names are nonempty and unique. Record graphs must be acyclic in
this first slice. A field type may use the already accepted primitive, record,
tuple, function, array, table, optional, builtin, and explicit-union vocabulary.
Class/interface fields and field defaults remain unsupported until their exact
construction and initialization metadata contracts are accepted; emission
fails closed rather than changing semantics.

The consumer indexes the record under its producer identity, reconstructs an
isolated session-local type and field schema, and carries a backend-neutral HIR
record reference into MIR. Ordinary record construction, field projection,
function parameters/results, and nested supported record types use that exact
schema. The consumer does not gain access to the producer Module or any
non-public declaration.

FFI layout records remain governed by ADR 0086. Their ordinary record schema is
shared, while target layout catalogs remain separately mandatory for public
foreign signatures. An ordinary record needs no FFI catalog and gains no ABI
claim merely by appearing in reference metadata.

## Consequences

- Core value libraries can expose native Pop records through normal `.poplib`
  dependencies.
- Record values retain nominal producer identity while compiler sessions use
  collision-free local type and field IDs.
- Metadata validation distinguishes ordinary record schemas from target FFI
  layout evidence.
- Generic records, defaults, nominal class/interface fields, recursive record
  graphs, methods, and runtime reflection are not silently introduced.

## Required conformance

- public records emit in identity order; `internal`/`private` records do not;
- exact field order, names, nested types, and producer identity survive
  canonical metadata round trips;
- a dependent Bubble names, constructs, projects, passes, and returns an
  ordinary record and lowers it to verified HIR/MIR;
- local producer/consumer `TypeId`, `FieldId`, and `SymbolId` collisions do not
  alter the producer identity;
- duplicate/missing/wrong-Bubble record identities, duplicate/empty fields,
  unsupported field types/defaults, cycles, and dangling record references fail
  before HIR;
- ordinary records require no FFI catalog, while records in public foreign
  signatures still require ADR 0086 target evidence; and
- no source loading, dynamic lookup, runtime map, reflection registry, or
  backend-specific HIR/MIR operation is added.

## Documents/components affected

Reference metadata emission/validation/loading, resolution, type checking,
HIR/MIR record schemas, deterministic `.poplib` encoding, artifact tests,
standard-library value families, architecture conformance, and the roadmap.
