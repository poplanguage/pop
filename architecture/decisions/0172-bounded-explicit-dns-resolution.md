# ADR 0172: Bounded Explicit DNS Resolution

- Status: accepted
- Date: 2026-08-03
- Depends on: ADRs 0152, 0159, and 0171

Native ABI 1.36 introduces explicit system-resolver capabilities and bounded
answer collections. Resolution requires a resolver handle, a compiler-proven
canonical `DnsName` string, and a nonzero maximum answer count. It deduplicates
answers in resolver order and preserves IPv4 versus IPv6 exactly.

Answer collections are opaque owned capabilities with checked count, family,
IPv4-word, and IPv6-word access. Callers explicitly close both resolver and
answer handles. Invalid handles, indices, families, limits, or strings fail
closed. DNS never occurs implicitly inside TCP or UDP operations.

This synchronous system adapter is the platform foundation. Public APIs expose
typed resolver and resolution handles; deadline, cancellation, cache policy,
record queries, DNS over TLS, and DNS over HTTPS remain explicit higher layers.
