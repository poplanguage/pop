# ADR 0164: Closed Runtime ABI Signatures

- Status: accepted
- Date: 2026-08-03

Each Atomic, local Actor, TCP, and UDP `RuntimeOperation` has one backend-neutral
native ABI signature. Signatures preserve exact scalar widths, distinguish
signed integers, and distinguish read-only byte spans from writable scalar and
byte output pointers. They contain no LLVM types, host Rust types, dynamic
arguments, variadic calls, or string-selected operations.

Compiler backends must consume these signatures when declaring and validating
native calls. TCP I/O uses ABI 1.28's closed socket status and separate byte
count outputs. Public Pop Lang APIs remain typed adapters above this closed ABI
and never expose its raw pointers.
