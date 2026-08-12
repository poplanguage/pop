# Pop HTTP server example

This is a complete TCP HTTP/1.1 server built only with `Pop.Standard`. It
accepts connections, reads request bytes, routes `/health` and `/api/info`,
writes responses, and closes each stream.

Build it with:

```text
pop build --manifestPath examples/httpServer/bubble.toml
```

The server listens on `127.0.0.1:8080`. The source deliberately uses the
Pop-shaped `Pop.Net` API; the socket ownership and byte transfers are provided
by the existing Rust native runtime.

`Pop.Standard` also exposes UDP, Unix sockets, DNS, interface and route
snapshots, deadlines/cancellation, multicast controls, TCP controls, and
Rustls-backed TLS through the same typed API.
