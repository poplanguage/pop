# ADR 0135: Bounded Portable Lexical Paths

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0030, 0031, 0032, 0110, 0123, and 0129
- Supersedes: the planned-only first `Path` catalog slice

## Context

File, directory, process, archive, resource, and glob APIs need a shared lexical
path before any filesystem authority is introduced. Raw Strings repeat
separator, dot-component, parent, name, and extension policy. Treating a lexical
path as proof of existence, access, or native encoding would mix a pure value
with host capabilities.

Ordinary public records can carry one owned canonical spelling between Bubbles.
The existing Text and List foundations are sufficient for bounded deterministic
normalization.

## Decision

`Pop.Path.Value` is an ordinary record containing canonical owned `text` and an
exact `absolute` Boolean. The portable spelling uses `/` separators. Complete
UTF-8 input is limited to 4,096 bytes and rejects NUL and backslash so drive,
UNC, and separator interpretation cannot depend on the host.

`normalize(String) -> Path.Value?` collapses duplicate separators, removes `.`,
removes a preceding ordinary component for `..`, clamps attempts above an
absolute root, and preserves unmatched leading `..` in a relative path. Empty
relative results canonicalize to `.`, and the absolute empty result is `/`.
Trailing separators are removed except for `/`. Unicode component spelling is
preserved exactly; no normalization, casing, or locale policy is implied.

The first remaining operations are:

- `format(Path.Value) -> String`;
- `isAbsolute(Path.Value) -> Boolean`;
- `join(Path.Value, String) -> Path.Value?`, which normalizes an appended
  relative spelling and rejects an absolute second argument;
- `parent(Path.Value) -> Path.Value?`, returning absence for `.` and `/`;
- `name(Path.Value) -> String?`, returning absence for `.` and `/`; and
- `extension(Path.Value) -> String?`, returning the final nonempty suffix after
  a non-leading period, or absence.

Join is lexical composition, not containment proof. Callers performing archive
extraction or rooted access must separately reject preserved leading traversal
and apply a capability-owned containment policy.

Native byte/UTF-16 paths, drive/UNC/device roots, target separators, relative
path computation between two values, component iteration, stems, multiple
extensions, filesystem canonicalization, symlink resolution, existence,
permissions, current-directory lookup, and ambient filesystem access remain
later typed contracts.

The implementation is ordinary Pop source with no PLRI call, host query,
runtime reflection, dynamic lookup, or backend-specific HIR/MIR operation.

## Consequences

- Downstream host APIs can share one deterministic portable lexical value.
- Security-sensitive callers can distinguish preserved relative traversal from
  an absolute path whose above-root traversal was clamped.
- Native path encoding remains a distinct typed platform concern instead of
  being lost through an implicit String conversion.

## Required conformance

- empty, dot, root, duplicate separator, dot-component, parent-component,
  Unicode, and trailing-separator cases normalize canonically;
- absolute above-root traversal clamps while unmatched relative traversal is
  preserved;
- NUL, backslash, and oversized input return absence;
- join rejects an absolute child and normalizes relative composition;
- parent, name, extension, format, and absolute inspection cover root/current,
  single/multiple component, dotfile, trailing-period, and Unicode cases;
- checked documentation and the frozen API baseline include the record and
  seven functions;
- the same ordinary source reaches verified HIR/MIR and executes on the MIR
  interpreter and LLVM backend; and
- no existence/access/symlink/native-encoding claim, implicit current
  directory, dynamic value, native duplicate, or backend-specific IR is added.

## Documents/components affected

System catalog, essential-library projection, closed decisions, standard
implementation plan, API baseline, ordinary `Pop.Standard` source,
documentation checks, and interpreter/LLVM conformance.
