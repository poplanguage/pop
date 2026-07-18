# ADR 0092: Private Semantic Definition Navigation

- Status: accepted
- Date: 2026-07-18
- Supersedes: none
- Amends: ADR 0089, ADR 0090

## Context

ADRs 0089 and 0090 authorize a version-coupled compiler tooling projection and
conservative same-Bubble Package snapshots. The private language server can
therefore prove which declaration a direct call selects, but it discards that
identity after producing parameter inlay hints. Editors cannot navigate even
between two Modules that the same compiler query has already resolved.

Definition navigation cannot be recovered safely from spelling, namespaces,
filenames, numeric IDs, or CLI output. Complete Workspace dependency loading,
public source and syntax schemas, local binding indexes, references, and rename
remain separate work.

## Decision

The private compiler tooling projection may expose resolved definition
occurrences. Each occurrence contains:

- the exact source selection span;
- the resolved `SymbolIdentity`; and
- no syntax, resolver, HIR, or MIR value.

The projection initially covers namespace-scope function declarations and
statically resolved direct function calls inside one analyzed Bubble. A
declaration name is an occurrence of its own identity. Unresolved, indirect,
referenced-dependency, method, member, local, and parameter uses do not produce
an occurrence in this slice.

The private language server implements LSP 3.17
`textDocument/definition`. It joins an occurrence to a declaration by
`SymbolIdentity`, returns the declaration selection range, and returns `null`
when either side is absent. The request reads the current immutable document
snapshot, checks cancellation, and uses UTF-16 positions. A definition in
another source-owned Module of the same selected Bubble may return that
Module's file URI.

One Bubble analysis snapshot owns a deterministic map from session `FileId`
values to source URIs and text. An already open Module keeps its session
`FileId`; a closed Module receives a deterministic snapshot-local ID. File
paths and URIs select and present source inputs only. They never establish
symbol identity, merge visibility, or replace the Item → Module → Bubble →
Package → Workspace hierarchy.

The existing ADR 0090 restrictions remain active. Package snapshots are used
only for conventionally discovered Bubbles without unresolved dependency
edges. Dependency references, sibling Bubbles, nested Packages, and editor
workspace folders are never guessed or merged. Complete Workspace snapshots
must later reuse the Package resolver and locked dependency graph rather than
extend this filesystem bootstrap heuristically.

References, rename, completion, signature help, local/member navigation,
cross-Bubble navigation, public `Pop.Syntax`/`Pop.Lsp` schemas, and incremental
range edits remain outside this decision.

## Consequences

- Same-Module and same-Bubble direct calls gain exact definition navigation.
- Navigation reuses compiler resolution instead of building a competing editor
  index.
- The snapshot retains enough source identity to present cross-Module
  locations while semantic identity remains path-independent.
- Unsupported or incomplete analysis fails closed with `null`.

## Alternatives considered

### Search names in open text

Rejected because spelling cannot prove overload selection, visibility,
shadowing, or Bubble identity.

### Navigate by namespace and filename

Rejected because neither value is semantic identity and Modules do not derive
identity from directory layout.

### Expose resolver or HIR nodes to the server

Rejected because compiler-private arenas and IR are unstable ownership
boundaries and are not public tooling schemas.

### Implement references and rename together

Rejected because multi-file edits, dependency indexes, locals, members, stale
snapshot handling, and atomic verification require a broader contract.

## Required conformance tests

- a declaration name and a same-Module direct call navigate to the exact
  declaration selection;
- a call in one Module navigates to a declaration in a sibling Module of the
  same dependency-free Bubble;
- UTF-16 request and result positions remain exact around non-BMP text;
- unresolved, indirect, dependency-bearing, stale, and closed snapshots do not
  fabricate a destination;
- two Packages or Bubbles with the same namespace never merge;
- the compiler projection joins occurrences and declarations only by
  `SymbolIdentity`; and
- advertised LSP capabilities exactly match the implemented request.

## Documents/components affected

CLI/tooling architecture, implementation roadmap, closed design decisions,
compiler driver tooling projections, private language-server snapshots and
transport, official editor extensions, and architecture conformance tests.
