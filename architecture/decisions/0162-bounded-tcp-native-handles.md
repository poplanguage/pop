# ADR 0162: Bounded TCP Native Handles

- Status: accepted
- Date: 2026-08-03

The native runtime exposes opaque TCP listener and stream handles for explicit
IPv4 loopback endpoints. Bind and connect accept a numeric port only; no DNS,
environment, global registry, or implicit host selection is permitted. Accept,
send, receive, and close fail closed on invalid handles and caller bounds.

The first ABI slice is a deterministic capability bridge for compiler/runtime
integration. TLS, cancellation, deadlines, public `Net.Tcp` records, and
non-loopback policy remain later layers.
