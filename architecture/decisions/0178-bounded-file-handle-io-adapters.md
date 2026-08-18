# ADR 0178: Bounded File-Handle I/O Adapters

- Status: Accepted
- Date: 2026-08-12
- Depends on: ADR 0097, ADR 0117, ADR 0177

## Context

ADR 0177 provides explicit file capabilities and opaque file handles. Direct
handle operations are useful as a bridge, but applications should not need to
reimplement bounded buffering and the common copy/read-all loops in every Pop
program.

## Decision

`Pop.Io` exposes two typed adapters over already-open `File.Handle` values:

- `Io.copy(source: File.Handle, destination: File.Handle, maximum: UInt64)
  -> Int` delegates bounded copying and returns the number of bytes written, or
  `-1` on stale handles, incompatible access modes, I/O failure, or a limit
  above 64 MiB;
- `Io.readAll(handle: File.Handle, maximum: UInt64) -> Bytes?` repeatedly reads
  at most 64 KiB at a time until EOF, returns independently owned bytes, and
  rejects limits above 64 MiB or any failed read.

Both adapters preserve the handle's current position, allocate only bounded
caller-visible result storage, and never close or replace the supplied handle.
They do not create filesystem authority, truncate or append files, seek, or
claim durable storage. Generic `Io.Reader`/`Io.Writer` protocols, seeking,
buffer pooling, suspension, and network-stream adapters remain separate
contracts.

## Required proof

The source adapters must compile in the Standard reference, execute in the MIR
interpreter, preserve stale-handle and maximum-limit rejection, and remain
available to native-linked programs through the existing handle operations.
The API baseline and Standard documentation must identify the bounded costs and
the dependency on explicit `File.Handle` ownership.

## Consequences

Programs gain the common high-level I/O loops without reimplementing them, while
filesystem authority and resource lifetime stay explicit at the call site.
