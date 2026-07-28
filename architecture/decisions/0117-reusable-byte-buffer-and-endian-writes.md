# ADR 0117: Reusable Byte Buffer and Endian Writes

- Status: accepted
- Date: 2026-07-28
- Depends on: ADRs 0030, 0031, 0032, 0034, 0051, 0055, 0097, 0110, and
  0113
- Supersedes: none

## Context

`Bytes.View` supplies allocation-free immutable inspection, but codecs,
protocols, compression, text encoding, and user libraries still cannot build
owned byte sequences incrementally without using `List<Byte>` as an accidental
public representation. Repeated immutable concatenation would copy the growing
prefix and turn ordinary construction into quadratic work.

The architecture reserves `Bytes.Buffer` for mutable reusable construction and
requires explicit growth and checked UTF-8 finishing. It does not yet define
the buffer identity, exact first API, aliasing, snapshot, capacity, integer
write, IR, or runtime contract. Reusing `List<Byte>`, `Array<Byte>`, or
`Ffi.Buffer<Byte>` would erase semantic boundaries and expose the wrong
ownership, mutation, or foreign-memory behavior.

## Decision

### Closed mutable identity

`Pop.Standard` appends bootstrap type identity 124, `Bytes.Buffer`.
It is an opaque final mutable reference type with stable identity and
encapsulated storage. It is not `Bytes`, `Bytes.View`, `List<Byte>`,
`Array<Byte>`, `Ffi.Buffer<Byte>`, an interface, a table, or an iterable.
Copying a buffer reference aliases the same mutable buffer. Buffer equality,
ordering, hashing, inheritance, literal construction, field access, and public
capacity inspection are not part of this contract.

The exact first public API is:

```luau
public function Bytes.create(): Bytes.Buffer
public function Bytes.withCapacity(capacity: Int): Bytes.Buffer
public function Bytes.length(buffer: Bytes.Buffer): Int
public function Bytes.reserve(buffer: Bytes.Buffer, additionalCapacity: Int): ()
public function Bytes.clear(buffer: Bytes.Buffer): ()

public function Bytes.write(buffer: Bytes.Buffer, value: Byte): ()
public function Bytes.write(buffer: Bytes.Buffer, value: Bytes): ()
public function Bytes.write(buffer: Bytes.Buffer, value: Bytes.View): ()

public function Bytes.writeUInt16BigEndian(buffer: Bytes.Buffer, value: UInt16): ()
public function Bytes.writeUInt16LittleEndian(buffer: Bytes.Buffer, value: UInt16): ()
public function Bytes.writeUInt32BigEndian(buffer: Bytes.Buffer, value: UInt32): ()
public function Bytes.writeUInt32LittleEndian(buffer: Bytes.Buffer, value: UInt32): ()
public function Bytes.writeUInt64BigEndian(buffer: Bytes.Buffer, value: UInt64): ()
public function Bytes.writeUInt64LittleEndian(buffer: Bytes.Buffer, value: UInt64): ()

public function Bytes.toBytes(buffer: Bytes.Buffer): Bytes
```

`create` starts empty with implementation-selected capacity.
`withCapacity` starts empty and reserves at least `capacity` bytes.
Negative capacity raises the closed `BoundsViolation` trap before allocation.
Capacity remains private so growth strategy can improve without changing
observable program results.

`length` returns the current byte count. `reserve` ensures that at least
`additionalCapacity` bytes can be appended without another growth operation,
measured from the current length. A negative additional capacity or checked
`length + additionalCapacity` overflow raises `BoundsViolation`. Reserving
never changes length or existing bytes and never shrinks storage.

`clear` sets length to zero and retains reusable capacity. It does not publish,
free, or replace the buffer identity. Later writes begin at byte one.

Every `write` appends in argument order. The `Byte` overload appends one byte.
The owned-`Bytes` and `Bytes.View` overloads append exactly the selected bytes
and retain neither input nor view. Empty input is a no-op. A write may grow the
buffer geometrically when the prior reserve is insufficient; that possibility
is part of its allocation/effect documentation. Growth preserves the complete
prefix or traps without a partial write. There is no API that lends a
`Bytes.View` from mutable buffer storage.

The integer functions append exactly 2, 4, or 8 bytes. Big-endian writes place
the most significant byte first; little-endian writes place it last. Every
unsigned value is accepted and the resulting bytes round-trip through the
corresponding ADR 0113 read at the write's starting index. Each integer write
performs one checked capacity decision and commits all bytes or none.

`toBytes` returns an immutable independent snapshot of the current prefix.
It copies exactly `length` bytes and leaves the buffer, its length, and its
capacity reusable and unchanged. Later buffer mutation cannot change the
snapshot. Empty snapshots use the ordinary canonical empty `Bytes` behavior.
Ownership transfer/reset and shared copy-on-write storage are not exposed by
this decision.

### Cost, effects, and concurrency

`create` and `withCapacity` allocate one buffer object/storage owner and may
reach a safe point. `length` is O(1), allocates nothing, and has no safe point.
`reserve` is O(n) only when growth copies the current prefix; otherwise it is
O(1). `clear` is O(1) for scalar byte storage and allocates nothing.

One-byte writes are amortized O(1). Owned/view writes and integer writes are
O(n) in appended bytes and perform at most one growth operation. `toBytes` is
O(n) in current length and allocates one immutable result. Native lowering
batches every owned/view/integer write into one runtime transition; it cannot
cross once per byte.

Mutating canonical operations carry `Allocates`, `MayTrap`, and `MayUnwind`
where growth may occur. MIR inserts a separate explicit GC safe point
immediately before an allocating mutation and roots the buffer plus every live
view lender; the native adapter itself cannot initiate GC. They do not suspend,
perform ambient I/O, use FFI, or dispatch through an interface or runtime name.
`length` has none of those effects.
`toBytes` allocates and reaches a safe point but does not mutate the buffer.

`Bytes.Buffer` is not concurrently safe. Sharing it across tasks, actors, or
threads requires a later accepted ownership/synchronization contract.
This decision adds no ambient buffer, global pool, thread-local cache, or
hidden lock.

### Typed compiler and canonical MIR

Source calls resolve the exact reserved type and overload identities. They
never fall back to member-name lookup, `List<Byte>`, an unchecked carrier, or a
user declaration shadowed by compiler behavior.

HIR preserves closed byte-buffer expressions for create, length, reserve,
clear, byte/owned/view writes, integer writes, and snapshot materialization.
Canonical MIR preserves corresponding backend-neutral operations with the
exact buffer and argument types, unsigned integer width, byte order,
allocation site, effects, and source origin. A buffer reference remains a
precise managed root across every growth or snapshot safe point.

The verifier rejects:

- any buffer operation on a non-`Bytes.Buffer` value;
- a non-`Byte` scalar write or mismatched integer width;
- a view write without exact `DoesNotRetain` provenance;
- partial, reordered, backend-specific, or dynamically selected byte order;
- a buffer represented as `List`, array, table, FFI storage, or raw pointer;
- missing roots, effects, allocation sites, or trap edges; and
- a view create/slice operation whose lender is a mutable buffer.

### Interpreter, native ABI, LLVM, and C

The MIR interpreter owns a distinct growable byte vector value and implements
the canonical operations directly. It never represents a buffer as
`MirValue::List`, immutable `Bytes`, or a view lender.

Native ABI 1.25 and production ABI 2.3 append a closed byte-buffer adapter
family for create, length, reserve, clear, byte/owned/view write, fixed-width
integer write, and immutable snapshot. Runtime buffer metadata is distinct
from List and FFI-buffer metadata. Every adapter validates the exact buffer
token, checked lengths/ranges, closed integer width/order, and output pointer
before mutation. Earlier ABI descriptors remain immutable.

LLVM lowers only the verified canonical operations to those exact adapters,
preserves the buffer root across growth/snapshot calls, and reloads writable
ABI 2 roots before reuse. It cannot lower a buffer to an LLVM vector, host
`Vec`, `List`, or byte pointer with backend-defined semantics.

The experimental C backend fails capability validation for every byte-buffer
MIR operation and emits no partial C or implicit fixed array.

### Artifacts, documentation, and compatibility

`Bytes.Buffer` type identity 124 and the exact callable overloads append to the
Standard API baseline and public reference metadata. The type is standard tier
and not in the prelude. Buffer calls in another Bubble resolve through verified
Standard metadata and the selected `.poplib` target implementation.

Public documentation records mutation, growth, allocation, copying, byte
order, bounds traps, thread confinement, complexity, and the no-view rule.
The ABI descriptors, MIR operation tags, runtime storage layout, capacity,
growth factor, allocation class, and metadata table are private implementation
facts.

Changing append order, snapshot independence, reserve meaning, byte order,
trap atomicity, view retention, or the exact nominal type is incompatible.
Changing private growth strategy is compatible when all observable behavior
and stabilized benchmark budgets remain satisfied.

Checked UTF-8 finishing, buffer-aware codecs, hexadecimal/base32/base64,
bit operations, pooling, ownership transfer, streams, and mutable-buffer views
remain separate contracts.

## Consequences

- Portable libraries gain one reusable byte construction primitive without
  quadratic immutable concatenation or a foreign/list representation leak.
- Endian writes compose exactly with the existing checked reads.
- Buffer reuse and snapshot copying are explicit at call sites.
- Text decoding and codecs can build on one stable mutable foundation.

## Alternatives considered

### Use `List<Byte>`

Rejected because List is a general collection with different identity,
iteration, capacity, element, artifact, and optimization contracts. Exposing it
would make a temporary representation part of every binary API.

### Use `Array<Byte>` or grow arrays

Rejected because arrays remain fixed-size under ADR 0034. Replacing one array
with another would expose ownership plumbing and repeated-copy hazards.

### Use `Ffi.Buffer<Byte>`

Rejected because FFI buffers are scoped foreign-layout resources with close,
borrow, and unsafe boundary rules. Ordinary portable byte construction cannot
depend on FFI or expose native memory semantics.

### Return a view or transfer storage on finish

Rejected for this slice because mutable-buffer lending needs a wider lifetime
proof, while observable transfer/reset or copy-on-write needs a separate
ownership and aliasing contract. An independent snapshot is simple and safe.

### Implement endian writes as repeated public byte writes

Rejected for native targets because it creates up to eight runtime transitions
and permits a partial prefix before a later growth failure. One canonical
fixed-width operation preserves atomicity and batching.

## Required conformance tests

- exact type/API baseline identities, overloads, non-prelude visibility, and
  cross-Bubble reference/artifact round trips;
- create/empty, reserved create, length, repeated reuse, clear-with-retained
  storage, and exact append-order positives;
- negative capacity/additional capacity, checked overflow, allocation failure,
  forged token, wrong source type, wrong integer width/order, and no-partial-
  mutation boundaries;
- byte, owned `Bytes`, full/subrange/empty view, and alias-lifetime writes with
  no retained lender;
- every 16/32/64-bit endian minimum, maximum, asymmetric, consecutive, and
  ADR 0113 round-trip case;
- independent empty/nonempty snapshots followed by buffer mutation;
- HIR/MIR construction, effect, allocation-site, root, verifier-corruption,
  no-List/no-FFI/no-pointer, and no-buffer-view tests;
- MIR-interpreter/LLVM differential execution plus native ABI 1.25/2.3
  negotiation and immutable-prior-descriptor tests;
- C fail-closed validation with no artifact; and
- target-labelled reproducible allocation/throughput benchmarks before any
  numeric performance budget is stabilized.

## Documents/components affected

Bootstrap identities, Standard API baseline and metadata, type checking, HIR,
MIR, verifiers, effects, interpreter, LLVM, experimental C validation, PLRI,
native ABI/runtime, GC roots, Standard documentation/examples, catalog,
implementation plan, roadmap, architecture regressions, and benchmarks.
