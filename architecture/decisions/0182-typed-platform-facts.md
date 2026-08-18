# ADR 0182: Typed Platform Facts

- Status: Accepted
- Date: 2026-08-12
- Depends on: ADR 0030, ADR 0031, ADR 0032

## Decision

`Pop.Platform` exposes two closed, ordinary Pop enums:

- `Platform.OperatingSystem`: `Unknown`, `Linux`, `Windows`, `Macos`,
  `Android`, `Ios`, `Web`, and `Posix`;
- `Platform.Architecture`: `Unknown`, `X86`, `X86_64`, `Arm`, `Arm64`,
  `Wasm32`, and `Wasm64`.

`Platform.operatingSystem()` and `Platform.architecture()` return these enums.
The host bridge supplies only the internal numeric facts
`nativeOperatingSystem()` and `nativeArchitecture()` as `Byte`; the public
adapters immediately close those values over the declared enum cases and map
unknown targets to `Unknown`.

The facts are observational and allocation-free. They do not expose target
strings, runtime feature lookup, compiler handles, or backend-specific
branches. The bridge uses the selected native target's compile-time facts, so
the MIR interpreter and native-linked execution share the same closed values
for their respective targets.

## Required proof

The enums and adapters must compile in the complete Standard reference. Native
byte facts require exact bootstrap identities and LLVM/MIR coverage; unknown
codes must map to `Unknown`. API and architecture catalogs must identify the
closed cases and reject string-probe substitutes.
