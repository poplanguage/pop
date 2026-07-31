# ADR 0141: Bounded Text Glob Patterns

- Status: accepted
- Date: 2026-07-31
- Depends on: ADRs 0030, 0031, 0032, 0110, and 0114
- Supersedes: the planned-only first `Glob` matcher slice

## Context

Text filtering and later path expansion need a small deterministic pattern
language that does not inherit regular-expression complexity or execute a
shell. Directory traversal additionally needs filesystem capability and must
not be smuggled into the pure matcher.

## Decision

`Pop.Glob.Pattern` is an explicitly compiled immutable class with private typed
tokens. `compile(String) -> Pattern?` accepts at most 1,024 UTF-8 bytes and
recognizes:

- `*` for zero or more Unicode scalar values;
- `?` for exactly one Unicode scalar value; and
- backslash followed by one scalar for a literal `*`, `?`, or backslash (and
  consistently for any other escaped scalar).

A trailing backslash and oversized pattern are rejected. Adjacent stars are
collapsed. `matches(Pattern, String) -> Boolean` matches the complete input,
rejecting text over 4,096 UTF-8 bytes. It uses bounded iterative greedy
backtracking only to the most recent star, with no recursion or exponential
state space.

This first slice is text-only. Path separator rules, `**`, character classes,
brace expansion, case policy, filesystem enumeration, symlink policy, and
ignore-file dialects remain later contracts. Expansion requires an explicit
`Directory` capability.

## Required conformance

- literals, empty input, stars, question marks, escaping, Unicode scalars, and
  complete-match behavior are covered;
- malformed/oversized patterns and oversized inputs fail deterministically;
- checked documentation and the API baseline contain the class and two
  functions;
- MIR interpreter and LLVM execute the same ordinary Pop implementation; and
- no regex engine, shell, filesystem, dynamic value, native duplicate, or
  backend-specific IR is introduced.

## Consequences

The standard library gains a useful bounded matcher without acquiring host
authority or promising later path-dialect behavior.
