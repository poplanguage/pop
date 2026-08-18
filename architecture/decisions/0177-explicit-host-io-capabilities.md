# ADR 0177: Explicit Host I/O Capabilities

- Status: Accepted
- Date: 2026-08-12
- Depends on: ADR 0030, ADR 0031, ADR 0032, ADR 0052, ADR 0058, ADR 0097,
  ADR 0117, ADR 0135
- Supersedes in part: the phase-3 host-I/O placement in the public library
  catalogs by replacing its planned-only status with this initial contract

## Context

Pop Standard needs useful host I/O for production programs, but a raw path
function would give every call ambient process-filesystem authority. The public
library architecture already reserves `File` and `Directory` for explicit
capabilities, typed handles, reusable buffers, and target-specific policy.

## Decision

The first host-I/O slice exposes explicit access capabilities:

- `File.Access` authorizes operations within one host-selected root and carries
  the target's path and symlink policy;
- `Directory.Access` authorizes directory enumeration and mutation within one
  host-selected root;
- resource handles are opaque, nominal, and explicitly closed;
- reads and writes use caller-owned `Bytes.Buffer` or immutable `Bytes` values;
- every operation has a bounded byte/entry limit and reports exact typed
  success or failure;
- no operation accepts an ambient process path as authority, changes the
  process current directory, or silently follows a symlink policy;
- unsupported targets reject through target capability checks rather than
  emulation.

The first file-resource slice fixes the following callable contract:

- `File.open(access: File.Access, relativePath: String) -> File.Handle` opens
  an existing regular file for reading inside `access`;
- `File.read(handle: File.Handle, destination: Bytes.Buffer, maximum: UInt64)
  -> Int` clears the caller-owned buffer, appends at most `maximum` bytes, and
  returns the byte count or `-1` on failure;
- `File.close(handle: File.Handle) -> Boolean` consumes the exact handle and
  returns false for a stale or already-closed handle.
- `File.openWrite(access: File.Access, relativePath: String) -> File.Handle`
  opens an existing regular file for writing without truncating it;
- `File.write(handle: File.Handle, source: Bytes.Buffer, maximum: UInt64)
  -> Int` copies at most `maximum` bytes from the caller-owned buffer at the
  current file position and returns the byte count or `-1` on failure.

The initial write contract does not create, truncate, append, or claim durable
storage; those policies remain explicit follow-up operations.

`Io.copyFiles(source: File.Handle, destination: File.Handle, maximum: UInt64)
-> Int` copies at most the bounded byte limit between already-open handles and
returns the number of bytes written or `-1`. It is the first concrete adapter
for the later `Io.Reader`/`Io.Writer` protocol; generic protocol dispatch and
blocking/suspending adapters remain separate contracts.

Directory enumeration uses a bounded snapshot resource:

- `Directory.list(access: Directory.Access, relativePath: String,
  maximumEntries: UInt64) -> Directory.Snapshot` captures sorted UTF-8 entry
  names inside the access root, rejecting limits above 65,536;
- `Directory.Snapshot.count(snapshot: Directory.Snapshot) -> UInt64` reports
  the captured count;
- `Directory.Snapshot.name(snapshot: Directory.Snapshot, index: UInt64)
  -> String?` returns a name only for an in-range index;
- `Directory.Snapshot.close(snapshot: Directory.Snapshot) -> Boolean` consumes
  the exact snapshot.
- `Directory.create(access: Directory.Access, relativePath: String) -> Boolean`
  creates one new directory below the access root;
- `Directory.remove(access: Directory.Access, relativePath: String) -> Boolean`
  removes one empty directory below the access root and never performs
  recursive deletion.

Writing, truncation, append, metadata, and atomic replacement remain separate
contracts because they add mutation, durability, and crash-consistency policy.

The compiler-known operation inventory is backend-neutral. MIR records handle
identity, ownership, bounds, effects, and failure edges; LLVM and the MIR
interpreter lower the same operations. Native implementations may use the host
filesystem, but native symbols and platform state do not enter PLRI.

Convenience `File.read`/`File.write` forms are permitted only as access-capable
functions that receive an explicit `File.Access`; they are not ambient global
functions. `Io` remains the protocol layer for readers, writers, streams, copy,
limits, buffering, and blocking/suspension; it does not issue filesystem
authority.

## Security and lifetime

Access creation is an explicit host boundary. The host chooses the root and
policy before Pop code receives the capability. Handles cannot be forged from
integers or resolved by runtime strings. Closing invalidates the exact handle;
stale handles fail closed. Buffers and views obey the existing ownership and
GC-root rules.

Package manifests may declare the host capability name through the exact
`requiredCapabilities` list (`environmentAccess`, `fileAccess`, and
`directoryAccess`). The declaration is carried into `.poplib` metadata and is
required before a package can use the initial host-I/O bridge. It is an
authorization requirement, not a filesystem root: the embedding/runtime entry
point must still supply the selected root and policy in the next ABI slice.

## Required proof

The implementation must provide positive, rejection, bounds, stale-handle,
cleanup, and cross-backend coverage; checked public documentation and examples;
target capability metadata; and an updated API baseline. No ambient path API may
remain as a competing public model.

## Consequences

Production Pop programs can use files and directories without reimplementing
host bindings, while capability ownership remains visible in the call site.
The first implementation is deliberately smaller than the complete phase-3
family; process spawning, environment snapshots, terminal streams, archives,
and compression require their own focused contracts.
