# ADR 0159: Bounded DNS Name Values

- Status: accepted
- Date: 2026-08-03

## Decision

`Pop.Net` adds a pure `DnsName` value for canonical ASCII DNS names. Parsing is
strict and bounded: names are nonempty, at most 253 characters, contain labels
of at most 63 characters, use lowercase ASCII letters/digits/hyphens, and do
not begin or end a label with a hyphen. The root name is not represented.

Formatting returns the stored canonical spelling. This value layer performs no
resolver lookup, reverse lookup, cache access, environment inspection, socket
operation, or host-name normalization. Resolver records and explicit resolver
capabilities remain later contracts.
