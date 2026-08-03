# Rustls 0.23.43 Dependency Review

- Review date: 2026-08-03
- Owner: `pop-runtime-native` secure transport adapter
- Result: approved for typed TLS client/server transport

## Selected surface

The root declarations are exact:

```toml
rustls = { version = "=0.23.43", default-features = false, features = ["ring", "std", "tls12"] }
rustls-platform-verifier = { version = "=0.7.0", default-features = false }
rcgen = { version = "=0.14.8", default-features = false, features = ["crypto", "ring"] }
```

Only `pop-runtime-native` inherits them. The default AWS-LC, logging,
compression, and post-quantum feature set is disabled. Ring supplies the
curated cryptographic provider; Rustls supports TLS 1.2 and TLS 1.3; the
platform verifier supplies host trust-store validation without exposing
certificates or verifier internals as Pop runtime values.
Rcgen is test-only and creates ephemeral loopback certificates so encrypted
client/server behavior is proven without committing generated key material.

## Boundaries

Rustls types remain private to the native adapter. PLRI and public Pop APIs use
opaque typed capability handles, owning bytes, explicit server names,
deadlines, cancellation, and closed outcomes. HIR and MIR contain no Rustls
types or protocol implementation details.

The adapter must not add an insecure verifier, accept invalid certificates,
silently disable hostname validation, enable early data by default, or expose
string-selected cipher suites. Test-only roots are explicit configuration and
cannot replace the platform verifier in the ordinary client constructor.

## Reproducibility and license

`Cargo.lock` records registry sources and checksums for the selected graph.
Rustls is Apache-2.0, ISC, or MIT; Rustls Platform Verifier and Rcgen are MIT or
Apache-2.0. Their enabled dependencies use permissive licenses compatible with
the Apache-2.0 runtime boundary. Version or feature changes require repeating
the graph, license, advisory, and capability review.
