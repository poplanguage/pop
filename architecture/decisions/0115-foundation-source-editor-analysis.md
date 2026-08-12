# ADR 0115: Foundation Source Editor Analysis

- Status: accepted
- Date: 2026-07-28
- Supersedes: none
- Amends: ADR 0090, ADR 0106
- Depends on: ADR 0009, ADR 0035

## Context

ADR 0106 supplies the compiler-verified published `Pop.Standard` reference to
normal editor analysis. Applying that rule while editing the reserved
`Pop.Standard` source itself creates a false self-reference: source declarations
and their published counterparts enter one lookup context. Exact overload calls
then fail and produce dependent unknown-local diagnostics. Applying the same
normal-Bubble rule while editing `Pop.Internal` also invents the forbidden
`Pop.Internal` to `Pop.Standard` dependency.

The compiler already validates the toolchain's exact foundation source graph:
`Pop.Internal` has no library dependency, and `Pop.Standard` has exactly one
`Pop.Internal` dependency. Editor analysis must preserve that graph without
turning reserved names into ambient user capabilities or claiming general
dependency resolution.

## Decision

The private language server recognizes a bounded foundation-source analysis
context only when the nearest regular Package manifest and conventional Bubble
match one of these exact shapes:

- `Pop.Internal`: one library Bubble, no Package, platform, development, or
  native dependency; or
- `Pop.Standard`: one library Bubble and exactly one non-optional registry
  dependency, alias `PopInternal`, version `0.1.0`, with no selected Bubble
  override, platform, development, or native dependency.

It analyzes every conventionally discovered source-owned Module in that Bubble,
using open-document snapshots before filesystem text. `Pop.Internal` receives
no library dependency or reference metadata. `Pop.Standard` receives the
reserved `Pop.Internal` Bubble dependency and does not receive its own published
`Pop.Standard` reference.

All other normal editor analysis continues to receive the verified published
`Pop.Standard` reference under ADR 0106. A near match, extra dependency,
different version, platform dependency, development dependency, native library,
non-library Bubble, symlink, or malformed manifest receives no foundation
privilege and follows the existing bounded fallback. This decision does not add
general dependency loading, Package cache access, implicit globals, or
cross-Bubble visibility.

## Consequences

- Foundation overloads and cross-Module calls use their current source Bubble
  exactly once.
- Editing `Pop.Internal` cannot see `Pop.Standard`.
- Editing `Pop.Standard` retains its accepted one-way dependency on
  `Pop.Internal` without a self-reference.
- Ordinary Packages keep using the compiler-verified published Standard
  contract.
- The private editor duplicates a small exact manifest-shape check until the
  reusable Workspace/dependency query replaces the bounded bootstrap path.

## Alternatives considered

### Ignore duplicate published candidates during overload resolution

Rejected because that would hide an invalid Bubble graph inside the type
checker and could weaken exact overload identity for every Package.

### Analyze only the active foundation Module

Rejected because one `.pop` file is a Module, not a Bubble, and foundation
Modules may call declarations owned by sibling Modules.

### Give every reserved-looking namespace foundation dependencies

Rejected because namespaces and source paths never grant Package/Bubble
identity or private dependency capabilities.

## Required conformance tests

- opening the repository `Pop.Standard` Math Module analyzes the complete
  current source Bubble without overload or dependent-local diagnostics;
- the source Bubble receives no published `Pop.Standard` self-reference;
- an exact `Pop.Internal` source context receives no `Pop.Standard` names;
- an ordinary standalone or Package Bubble still receives the verified
  published `Pop.Standard` reference;
- near-match manifests and non-library Bubbles receive no foundation
  privilege; and
- open sibling Module snapshots participate in the same foundation Bubble
  reanalysis without merging another Package.

## Documents/components affected

Private language-server analysis, compiler tooling Bubble identities, CLI and
tooling architecture, architecture conformance policy, closed decisions, and
foundation editor regression tests.
