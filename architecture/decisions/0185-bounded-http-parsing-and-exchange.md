# ADR 0185: Bounded HTTP Parsing and Exchange

- Status: Accepted
- Date: 2026-08-13
- Depends on: ADRs 0110, 0133, 0143, 0169, 0183, and 0184

## Context

ADR 0183 serializes and sends typed HTTP/1.1 values, but an application still
has to parse received text manually. A useful protocol Package needs a bounded
request/response receive path while preserving explicit transport ownership,
limits, and failure.

## Decision

`Pop.Http` adds deterministic `parseRequest` and `parseResponse` functions for
one complete UTF-8 HTTP/1.1 message and `receiveResponse`/`exchange` adapters
over a caller-owned `Net.Tcp.Stream`.

Parsing accepts exactly an HTTP/1.1 start line, CRLF-delimited validated
headers, one mandatory valid `Content-Length`, and exactly that many UTF-8 body
bytes. The caller supplies a maximum body size no greater than 16 MiB. Duplicate
or conflicting content lengths, transfer encoding, malformed lines, status
outside 100 through 599, truncated bodies, trailing bytes, and oversized input
fail closed as `nil`.

`receiveResponse` reads reusable bounded chunks until the peer closes, rejecting
responses beyond the caller's header-plus-body budget. `exchange` sends one
request, shuts down the write half, then receives one response. Neither
function closes the stream, performs DNS, opens a connection, retries, follows
redirects, selects TLS, pools resources, or hides suspension.

## Required proof

- canonical requests and responses parse to exact typed fields and round-trip;
- injection, malformed start lines and headers, duplicate lengths, unsupported
  transfer encoding, truncation, trailing bytes, and every limit boundary fail
  closed;
- a loopback client and server exchange one HTML response through the public
  `Pop.Http` Package;
- dependent-Bubble checks and native linking preserve enum/record identities;
  and
- interpreter and LLVM execute the same portable parser behavior.

## Documents and components affected

`Pop.Http`, its API baseline and tests, HTTP examples, network catalog, package
artifacts, interpreter/LLVM differential tests, and documentation.
