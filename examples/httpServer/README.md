# Pop HTTP server example

This is a complete TCP HTTP/1.1 capability server built with `Pop.Http` and
`Pop.Standard`. It accepts connections, reads request bytes, routes typed
requests, writes responses, and closes each stream.

Build it with:

```text
pop build --manifestPath examples/httpServer/bubble.toml
pop run --manifestPath examples/httpServer/bubble.toml
```

The server listens on `127.0.0.1:18080`. In another terminal:

```text
curl http://127.0.0.1:18080/
curl http://127.0.0.1:18080/api/info
curl http://127.0.0.1:18080/api/network
curl http://127.0.0.1:18080/api/deadline
curl http://127.0.0.1:18080/api/host
curl http://127.0.0.1:18080/api/file
```

The routes demonstrate the typed HTTP/TCP path, process facts, environment
queries, scoped file access, bounded reads, IPv4 parsing/classification, and
monotonic deadline cleanup. The source deliberately uses Pop-shaped APIs; the
socket ownership and byte transfers are supplied by the native runtime.

`Pop.Standard` also exposes UDP, Unix sockets, DNS, interface and route
snapshots, deadlines/cancellation, multicast controls, TCP controls, and
Rustls-backed TLS through the same typed API.
