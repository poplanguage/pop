# ADR 0131: Bounded Semantic Version Values

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0030, 0031, 0032, 0058, 0110, 0123, and 0129
- Supersedes: the planned-only `Version` catalog row

## Context

Package manifests, tools, and applications need one portable semantic-version
value before package-resolution policy is designed. Leaving versions as
unvalidated Strings would repeat parsing, permit inconsistent precedence, and
encourage runtime string dispatch. A class would add identity and lifecycle to
an immutable value. An unrestricted range-expression language would silently
decide package solver policy.

The accepted ordinary-public-record metadata path can carry an immutable value
between Bubbles. The existing Text, Unicode, Bytes, and Math foundations are
enough for a deterministic pure Pop implementation.

## Decision

`Pop.Version.Value` is an ordinary public record containing nonnegative `major`,
`minor`, and `patch` Int components plus owned `prerelease` and `build` Strings.
Values produced by `parse` satisfy Semantic Versioning 2.0.0 syntax. Each core
component is at most 2,147,483,646, complete input is at most 1,024 UTF-8 bytes,
and prerelease/build identifiers use only ASCII letters, digits, and hyphens.
Empty identifiers, core leading zeros, and numeric prerelease leading zeros are
rejected. These bounds make parser work deterministic and keep compatible-range
upper bounds representable.

The first API is:

- `parse(String) -> Version.Value?`, which consumes the complete input without
  trimming or aliases;
- `format(Version.Value) -> String`, which emits `major.minor.patch`, then
  nonempty prerelease/build fields with their canonical separators;
- `compare(Version.Value, Version.Value) -> Int`, which returns negative one,
  zero, or positive one by Semantic Versioning precedence and ignores build
  metadata; and
- `matches(Version.Value, String) -> Boolean`, which accepts one complete exact,
  `=`, `>`, `>=`, `<`, `<=`, caret, or tilde requirement and returns false for
  malformed or oversized requirements.

Prerelease comparison follows the Semantic Versioning ordering: a release sorts
after a prerelease; numeric identifiers sort before nonnumeric identifiers;
numeric identifiers compare by magnitude without integer conversion; otherwise
ASCII lexical order applies; and a shared prefix sorts before the longer list.

An unprefixed or `=` requirement uses precedence equality, so build metadata is
ignored. Tilde admits values from the target through, but not including, the
next minor version. Caret admits values through, but not including, the next
major when major is nonzero, the next minor when major is zero and minor is
nonzero, or the next patch otherwise. Requirement intersections, unions,
wildcards, partial versions, package selection, prerelease opt-in policy, and
lockfile resolution remain package-tooling decisions.

The implementation is ordinary Pop source and uses no PLRI call, global state,
runtime reflection, dynamic lookup, or backend-specific HIR/MIR operation.
Manual record initializers are responsible for the same field invariants;
public operations remain deterministic for every statically typed record value,
but only parsed/validated values are canonical semantic versions.

## Consequences

- Core code shares one exact bounded parser, formatter, precedence relation, and
  small single-requirement matcher.
- Prerelease and build storage is owned and survives source-buffer lifetimes.
- Build metadata round-trips but never affects ordering or matching equality.
- The immutable record crosses Bubble boundaries through ADR 0129 rather than
  source loading or a runtime table.
- A future package solver can build its own typed requirement model without
  changing this value API.

## Required conformance

- valid core, prerelease, and build forms parse and format canonically;
- malformed separators, identifiers, leading zeros, non-ASCII identifiers,
  oversized text, and out-of-range components return absence;
- the complete Semantic Versioning prerelease precedence chain orders exactly,
  build metadata compares equal, and component comparison cannot overflow;
- exact, comparison, caret, and tilde requirements accept/reject at their
  boundaries while malformed requirements return false;
- documentation and the frozen standard API baseline include the record and
  four functions;
- the same ordinary source reaches verified HIR/MIR and executes identically on
  the MIR interpreter and LLVM backend; and
- no package solver, ambient capability, runtime string resolution, dynamic
  value, reflection registry, or backend-specific IR enters the implementation.

## Documents/components affected

Core library catalog, essential-library projection, closed decisions, standard
implementation plan, API baseline, ordinary `Pop.Standard` source,
documentation checks, and interpreter/LLVM conformance.
