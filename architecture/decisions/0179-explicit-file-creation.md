# ADR 0179: Explicit File Creation

- Status: Accepted
- Date: 2026-08-12
- Depends on: ADR 0177

## Context

The initial file contract can inspect, read, and write existing files, but it
cannot create a new output without asking an application to leave the explicit
capability model or reimplement a native bridge.

## Decision

`File.create(access: File.Access, relativePath: String) -> File.Handle` creates
one new regular file below the supplied access root and returns a writable
handle. It uses create-new semantics: an existing path, directory, traversal,
absolute path, symlink, or non-regular target fails closed. The operation does
not create parent directories, truncate an existing file, append, or claim
durability. The returned handle is closed with the existing `File.close` and is
usable by `File.write` and `Io.copyFiles`.

The operation requires the package's explicit `fileAccess` capability and is
represented by the same backend-neutral Standard operation inventory as the
other file-handle functions.

## Required proof

Native and MIR implementations must reject existing/traversal targets, create
and write a bounded new file in an isolated test, preserve explicit handle
ownership, and keep LLVM lowering and capability validation synchronized. The
API and documentation baselines must carry the exact signature and effects.

## Consequences

Pop programs can create output files without reimplementing host bindings,
while overwrite, append, parent creation, and durability remain visible future
contracts rather than hidden `open` options.
