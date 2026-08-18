# ADR 0183: Bounded HTTP Core

- Status: Accepted
- Date: 2026-08-12
- Depends on: ADR 0110, ADR 0133, ADR 0143, ADR 0162, ADR 0178, ADR 0184

## Decision

The independently versioned `Pop.Http` Package provides a typed HTTP/1.1 core
over an existing `Net.Tcp.Stream` from `Pop.Standard`.
The first portable slice contains `Method`, `Header`, `Request`, and
`Response` values, validation against header-injection line breaks, canonical
request/response serialization, automatic bounded `Content-Length`, and
complete send adapters that preserve the caller-owned stream.

The core does not own DNS, connection pools, redirects, TLS, task scheduling,
or unbounded bodies. Those capabilities remain explicit future layers. Bodies
are currently UTF-8 text values; binary streaming and multipart bodies require
separate typed contracts.

## Required proof

- malformed targets, reasons, and header values are rejected;
- request and response serialization is deterministic and includes a length
  when the caller did not provide one;
- sending uses the existing typed TCP transfer outcomes;
- the HTTP server example compiles through the public `Pop.Http` adapter.
- `Pop.Http` emits and is consumed through its own `.poplib`; none of its public
  Items enter the reserved `Pop.Standard` API baseline.
