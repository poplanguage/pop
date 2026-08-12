# ADR 0118: Checked UTF-8 Transcoding

- Status: accepted
- Date: 2026-07-28
- Depends on: ADRs 0032, 0041, 0051, 0097, 0110, 0114, 0116, and 0117
- Supersedes: none

## Context

Pop Lang `String` values are always valid UTF-8, while `Bytes` may contain any
octets. User codecs, parsers, network protocols, and text algorithms need one
checked boundary between those types. Source concatenation cannot safely decode
arbitrary bytes, and constructing text through repeated one-rune Strings would
allocate quadratically.

ADR 0117 supplies reusable byte construction but deliberately leaves checked
UTF-8 finishing separate. Treating malformed input as a runtime trap would make
ordinary parser input indistinguishable from a compiler/runtime invariant
failure. Reusing a native host string conversion without canonical HIR/MIR
would also create backend disagreement.

## Decision

`Pop.Text` appends four non-prelude overloads:

```luau
public function Text.encodeUtf8(value: String): Bytes
public function Text.encodeUtf8(value: Text.View): Bytes
public function Text.decodeUtf8(value: Bytes.View): String?
public function Text.decodeUtf8(value: Bytes.Buffer): String?
```

`encodeUtf8` returns the exact UTF-8 code units of the complete owned String or
selected Text view as independent immutable `Bytes`. Text-view boundaries are
already scalar boundaries, so encoding never splits a UTF-8 sequence.

`decodeUtf8` validates the complete selected byte range. It returns an
independent immutable `String` when the range is valid UTF-8 and `nil` when it
is malformed. Empty input is valid and produces the empty String. It never
replaces malformed sequences, ignores a byte-order mark, normalizes text, or
performs locale-sensitive conversion.

The `Bytes.Buffer` overload observes its complete current contents without
clearing, consuming, freezing, or otherwise mutating the buffer. Later buffer
writes do not change an already returned String, and a failed decode leaves the
buffer unchanged.

## Failure, allocation, and cost

Malformed UTF-8 is expected data and is represented only by optional absence.
Allocation exhaustion or a runtime invariant failure remains the ordinary
typed panic/unwind path; it is never reported as malformed input.

Every successful operation allocates exactly one independent immutable result
payload in the first implementation. Invalid decoding performs no result
allocation. All operations are O(n) in selected UTF-8 bytes with O(1)
additional validation state. They do not schedule, block, suspend, consult
ambient state, invoke user dispatch, or retain borrowed views.

## IR and runtime contract

Typed bodies, HIR, and canonical MIR carry distinct backend-neutral operations
for Text-view UTF-8 encoding, Bytes-view UTF-8 decoding, and buffer UTF-8
decoding. The owned-String encode overload creates only a compiler-proven
ephemeral Text view. Operands retain exact types and view lender provenance;
decode results are the ordinary static `String?` union.

The MIR interpreter validates and copies through the runtime adapter. LLVM uses
three typed native operations. The experimental C backend rejects these
runtime-dependent operations before emission. No backend performs source-name
lookup, host-language fallback, unchecked reinterpretation, or replacement
decoding.

Stable native ABI 1.26 and production ABI 2.4 append:

```text
pop_rt_text_view_encode_utf8(string, byteOffset, byteLength) -> Bytes
pop_rt_bytes_view_decode_utf8(bytes, byteOffset, byteLength, outString) -> u8
pop_rt_byte_buffer_decode_utf8(buffer, outString) -> u8
```

The encode operation returns a nonzero managed Bytes token or zero for a
runtime failure. Decode statuses are closed: `0 = runtime failure`,
`1 = malformed`, and `2 = valid`. Status 2 writes one nonzero managed String
token. Status 0 or 1 leaves the output slot unchanged. This separates malformed
data from allocation failure and avoids target-dependent aggregate return
layout.

Encode/decode allocation is preceded by an explicit canonical MIR GC safe
point. Exact String/Bytes lenders and live `Bytes.Buffer` inputs are published
as roots and are reloaded under ABI 2 before the operation. Result values then
participate in ordinary precise root tracking.

## Compatibility and artifacts

The four callable identities append to the Standard API baseline as
`implemented`; they do not enter the prelude. Calls in dependent Bubbles resolve
through verified public reference metadata and the selected `.poplib`
implementation.

ABI 1.11 through 1.25 and ABI 2.0 through 2.3 remain immutable descriptors.
The default facade supports compatible ABI 1 through 1.26 and rejects ABI 2;
the production facade supports compatible ABI 2 through 2.4 and rejects ABI 1.

## Consequences

- Text, URI, JSON, XML, HTTP, and user codecs gain one checked binary/text
  boundary.
- Reusable byte construction can finish as valid String without an intermediate
  immutable Bytes snapshot.
- Invalid UTF-8 remains explicit static optional flow.
- Normalization, Unicode properties, other encodings, streaming transcoders,
  and hexadecimal/base32/base64 remain separate library contracts.

## Alternatives considered

### Return replacement text

Rejected because silent replacement loses byte fidelity and hides malformed
protocol input.

### Trap on malformed input

Rejected because malformed external data is expected and must not become a
runtime invariant failure.

### Consume or clear the buffer

Rejected because ADR 0117 defines reusable aliased mutable storage and exposes
no ownership-transfer operation.

### Decode through a host-library-only helper

Rejected because interpreter, LLVM, and future VM behavior must follow one
canonical typed MIR contract.

## Required conformance tests

- owned String and full/subrange Text-view encodes cover empty, ASCII, two-,
  three-, and four-byte scalars plus embedded U+0000;
- Bytes-view and buffer decodes cover the same valid data and malformed
  truncated, continuation, overlong, surrogate, and above-maximum sequences;
- malformed decoding returns absence without mutation or result allocation;
- returned values remain independent after source or buffer reuse;
- wrong types and arities fail statically without fallback;
- HIR/MIR verification rejects operand, result, lender, effect, root, and
  allocation-site drift;
- interpreter and real LLVM execution agree;
- ABI 1.26/2.4 symbols, closed statuses, root relocation, and prior descriptor
  compatibility pass;
- the C backend fails closed; and
- architecture tests reject dynamic conversion, replacement decoding,
  intermediate mandatory Bytes materialization for buffer decode, and
  compiler/backend source-name lookup.

## Documents/components affected

Text/Bytes catalog and examples, closed decisions, conformance policy, Standard
API baseline, type checking, HIR, MIR, runtime interface, interpreter, LLVM,
native ABI/runtime, C validation, architecture tests, roadmap, and performance
evidence.
