# ADR 0180: Process Executable Value

- Status: Accepted
- Date: 2026-08-12
- Depends on: ADR 0025, ADR 0032

## Context

Process identity and argument inspection are useful for diagnostics and command
line tools, but a program should not need to reimplement the host bridge to
discover the executable selected for the current process.

## Decision

`Process.executable() -> String?` returns the host-reported executable path as
an owned string, or nil when the target cannot provide a valid UTF-8 path. The
operation is observational: it does not resolve or execute a command, change
the working directory, or grant filesystem authority. It carries `AmbientIo`,
`Allocates`, and `MayTrap` effects and needs no process-spawn capability.

The value is exposed through the same typed Standard bridge and optional-value
ABI used by other managed strings. Target-specific path encoding failure is
represented by absence rather than lossy replacement.

## Required proof

The native bridge, MIR interpreter, and LLVM lowering must agree on the exact
optional string representation. The API and function baselines, embedded
Standard metadata, and a positive/absence boundary test must be synchronized.

## Consequences

Diagnostics and command-line programs can identify their executable without
ambient command execution. Spawn, child handles, environment construction, and
working-directory policy remain separate explicit contracts.
